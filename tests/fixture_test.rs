//! Parses envelopes captured from a real `fxv` binary (see `tests/fixtures/`) with the typed
//! structs, and re-serializes them to prove the field/omission rules match the CLI byte-for-byte
//! (as JSON values). Fixtures were captured from a local-only workspace; regenerate them with a
//! current `fxv` build if the wire format changes.
//!
//! Payloads are verbatim. The `program` block's `executable`, `arguments[0]`, and
//! `working_directory` are rewritten to a neutral path when a fixture is added, because the CLI
//! reports the real invocation and there is no reason to publish whoever captured it.
//!
//! Conflicts need no server. `status_parented_draft.json`, `sync_conflict.json`,
//! `status_conflict.json` and `resolve.json` come from one local-only sequence: `user add alice`,
//! `login alice` (publish needs a user), publish twice, `goto main.0`, edit one file and delete
//! another, `snapshot` (the parented-draft fixture), `sync main.1`, `resolve --mine alpha.txt`.
//!
//! `status_type_change_conflict.json` is the same sequence but publishes a path as a **file** and
//! replaces it with a **directory** on the draft side. The other direction still carries a change
//! axis, so it does not produce the axis-less entry this fixture exists for.

// == Std
use std::path::Path;

// == Internal crates
use fxv_api::v1::{
    change_info::ChangeInfoOutputJson,
    common::{ChangeKind, ConflictKind},
    envelope::{ExitCode, MessageVersion, OutputEnvelope, ParsedOutput, parse_output},
    history::HistoryOutputJson,
    init::InitJson,
    interrupted_sync::{InterruptedOperation, InterruptedSyncJson},
    login::LoginJson,
    logout::LogoutJson,
    status::{HeadCommitJson, StatusJson},
    workspace_sync::WorkspaceSyncJson,
};

// == External crates
use jsonschema::{Resource, Validator};
use serde_json::Value;

fn read_fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read fixture {name}: {e}"))
}

fn load_schema(file_name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas").join(file_name);
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

/// Compiles the envelope schema with every payload schema registered (same set the CLI's own
/// tests use).
fn build_envelope_validator() -> Validator {
    const SCHEMA_RESOURCES: &[(&str, &str)] = &[
        ("urn:fxv:schema:changeinfo:v1", "changeinfo.schema.json"),
        ("urn:fxv:schema:common:v1", "common.schema.json"),
        ("urn:fxv:schema:doctor:v1", "doctor.schema.json"),
        ("urn:fxv:schema:error:v1", "error.schema.json"),
        ("urn:fxv:schema:history:v1", "history.schema.json"),
        ("urn:fxv:schema:init:v1", "init.schema.json"),
        ("urn:fxv:schema:interrupted-sync:v1", "interrupted_sync.schema.json"),
        ("urn:fxv:schema:login:v1", "login.schema.json"),
        ("urn:fxv:schema:logout:v1", "logout.schema.json"),
        ("urn:fxv:schema:status:v2", "status.schema.json"),
        ("urn:fxv:schema:upgrade:v1", "upgrade.schema.json"),
        ("urn:fxv:schema:user:v1", "user.schema.json"),
        ("urn:fxv:schema:workspace-sync:v1", "workspace_sync.schema.json"),
    ];
    let mut options = jsonschema::options();
    for (urn, file_name) in SCHEMA_RESOURCES {
        options.with_resource(*urn, Resource::from_contents(load_schema(file_name)).expect(file_name));
    }
    options
        .build(&load_schema("envelope.schema.json"))
        .expect("Failed to compile envelope schema")
}

/// Parses a success fixture as `T`, checks the envelope kind, validates the raw fixture against
/// the schemas, and asserts the typed re-serialization is value-identical to what the CLI wrote.
fn check_success_fixture<T>(file: &str, kind: &str) -> OutputEnvelope<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let text = read_fixture(file);

    let raw: Value = serde_json::from_str(&text).unwrap();
    let validator = build_envelope_validator();
    let errors: Vec<_> = validator.iter_errors(&raw).collect();
    assert!(errors.is_empty(), "{file} fails schema validation: {errors:?}");

    let envelope = match parse_output::<T>(&text).unwrap_or_else(|e| panic!("parse {file}: {e}")) {
        ParsedOutput::Success(envelope) => envelope,
        ParsedOutput::Error(e) => panic!("{file} parsed as error: {}", e.message.payload.message),
    };
    assert_eq!(envelope.message.kind, kind, "unexpected kind in {file}");

    let reserialized = serde_json::to_value(&envelope).unwrap();
    assert_eq!(
        reserialized, raw,
        "typed round-trip of {file} does not match the CLI's output"
    );
    assert_manifest_covers(kind, envelope.message.version, file);
    envelope
}

/// `schemas/message_versions.json` is the CLI's published kind -> version table, mirrored here and
/// generated from downstream. A fixture carries a version the CLI really emitted, so it catches a
/// row the schema copy alone would not. Older fixtures are fine; ahead of the manifest is not.
fn assert_manifest_covers(kind: &str, captured: MessageVersion, file: &str) {
    let manifest = load_schema("message_versions.json");
    let raw = manifest["versions"][kind]
        .as_str()
        .unwrap_or_else(|| panic!("kind {kind:?} is missing from message_versions.json"));
    let listed: MessageVersion = raw.parse().expect("manifest versions are major.minor");
    assert!(
        (listed.major, listed.minor) >= (captured.major, captured.minor),
        "{file} carries {kind} {captured}, ahead of message_versions.json at {listed} - re-mirror it"
    );
}

#[test]
fn test_init_fixture() {
    let envelope = check_success_fixture::<InitJson>("init.json", "init");
    assert!(envelope.message.payload.local_only);
    assert_eq!(envelope.message.payload.repo_uri, None);
}

#[test]
fn test_status_fixture() {
    let envelope = check_success_fixture::<StatusJson>("status.json", "status");
    let status = &envelope.message.payload;
    assert_eq!(status.current_branch, "main");
    assert!(matches!(status.head_commit, HeadCommitJson::UnparentedDraft { .. }));
    // Holds only because nothing here is conflicted; see the type-change fixture.
    assert_eq!(status.file_change_counts.total, status.files.len());
}

/// A parented draft behind its published head: what the publish flow has to notice first.
#[test]
fn test_status_parented_draft_fixture() {
    let envelope = check_success_fixture::<StatusJson>("status_parented_draft.json", "status");
    let status = &envelope.message.payload;
    assert_eq!(envelope.message.version, MessageVersion::new(2, 0));
    assert_eq!(status.current_user.as_deref(), Some("alice"));

    let HeadCommitJson::ParentedDraft {
        local_snapshot,
        published_head,
    } = &status.head_commit
    else {
        panic!("the fixture was captured on a draft with a published parent");
    };
    assert_eq!(local_snapshot.commit.branch, "main");
    assert_eq!(published_head.commit.revision, Some(1));

    let sync = status
        .sync_status
        .as_ref()
        .expect("a parented draft reports sync_status");
    assert!(!sync.up_to_date);
    assert_eq!(sync.revisions_behind, 1);
    assert_eq!(sync.published_head_revision, 1);
    assert_eq!(sync.synced_revision, Some(0));

    // Behind the remote, but nothing conflicts until the sync runs.
    assert!(status.files.iter().all(|file| file.conflict_state.is_none()));
}

/// Syncing the divergent draft onto `main.1`: an edit and a delete/edit collision, which pins
/// `conflict_state.kind` to a real value rather than a presence check.
#[test]
fn test_status_conflict_fixture() {
    let envelope = check_success_fixture::<StatusJson>("status_conflict.json", "status");
    let status = &envelope.message.payload;
    assert_eq!(envelope.message.version, MessageVersion::new(2, 0));

    let alpha = status
        .files
        .iter()
        .find(|file| file.path == "alpha.txt")
        .expect("alpha.txt is in conflict");
    assert_eq!(alpha.unpublished_state, Some(ChangeKind::Modified));
    assert_eq!(
        alpha.conflict_state.map(|state| state.kind),
        Some(ConflictKind::Content)
    );

    // A conflicted deletion keeps its change axis.
    let beta = status
        .files
        .iter()
        .find(|file| file.path == "beta.txt")
        .expect("beta.txt is in conflict");
    assert_eq!(beta.unpublished_state, Some(ChangeKind::Deleted));
    assert_eq!(beta.conflict_state.map(|state| state.kind), Some(ConflictKind::Deleted));
}

/// The shape the bump added: a conflicted path on **neither** change axis, counted in `total` by
/// neither. Walking the two axes drops it.
#[test]
fn test_status_type_change_conflict_fixture() {
    let envelope = check_success_fixture::<StatusJson>("status_type_change_conflict.json", "status");
    let status = &envelope.message.payload;

    let clash = status
        .files
        .iter()
        .find(|file| file.path == "gamma")
        .expect("the clashing path is reported");
    assert_eq!(clash.unpublished_state, None);
    assert_eq!(clash.workspace_state, None);
    assert_eq!(
        clash.conflict_state.map(|state| state.kind),
        Some(ConflictKind::TypeChange)
    );

    let counts = &status.file_change_counts;
    assert_eq!(counts.total, status.files.len());
    assert!(
        counts.total > counts.unpublished + counts.workspace_need_snapshot,
        "an entry on neither axis is still counted in total"
    );
    assert_eq!(status.conflicted_files().count(), 1);
    assert_eq!(
        status.unpublished_files().chain(status.workspace_files()).count(),
        status.files.len() - 1,
        "walking the two axes misses the conflict-only entry"
    );
}

#[test]
fn test_login_fixture() {
    let envelope = check_success_fixture::<LoginJson>("login.json", "login");
    assert_eq!(envelope.message.payload.username, "alice");
}

#[test]
fn test_history_fixture() {
    let envelope = check_success_fixture::<HistoryOutputJson>("history.json", "history");
    assert!(!envelope.message.payload.entries.is_empty());
}

#[test]
fn test_changeinfo_fixture() {
    let envelope = check_success_fixture::<ChangeInfoOutputJson>("changeinfo.json", "changeinfo");
    let info = &envelope.message.payload;
    assert_eq!(info.summary.total_changed, info.changes.len());
}

#[test]
fn test_logout_fixture() {
    check_success_fixture::<LogoutJson>("logout.json", "logout");
}

/// The workspace-sync family shares one payload under three different kinds, so each is parsed
/// separately: only `revert` and `goto` here created a pre-sync snapshot, and only `revert` was
/// given a path rather than a whole revision.
#[test]
fn test_sync_fixture() {
    let envelope = check_success_fixture::<WorkspaceSyncJson>("sync.json", "sync");
    let payload = &envelope.message.payload;
    assert_eq!(payload.target_revision, "main.-.4");
    assert!(payload.files_updated.is_empty(), "a sync with nothing to apply");
    assert_eq!(payload.error_count, 0);
}

#[test]
fn test_goto_fixture() {
    let envelope = check_success_fixture::<WorkspaceSyncJson>("goto.json", "goto");
    let payload = &envelope.message.payload;
    assert_eq!(payload.target_revision, "main.-.2");
    assert_eq!(payload.files_updated_count, payload.files_updated.len());
    assert!(
        !payload.created_revisions.is_empty(),
        "a goto snapshots the workspace before moving it"
    );
}

#[test]
fn test_revert_fixture() {
    let envelope = check_success_fixture::<WorkspaceSyncJson>("revert.json", "revert");
    let payload = &envelope.message.payload;
    assert_eq!(payload.files_updated_count, 1);
    assert!(payload.conflicted_files.is_empty());
}

/// A sync it cannot merge cleanly: nothing updated, conflicts under `conflicted_files` rather
/// than as an error.
#[test]
fn test_sync_conflict_fixture() {
    let envelope = check_success_fixture::<WorkspaceSyncJson>("sync_conflict.json", "sync");
    let payload = &envelope.message.payload;
    assert_eq!(payload.target_revision, "main.1.1");
    assert_eq!(payload.files_updated_count, 0);
    assert_eq!(payload.error_count, 0, "a conflict is not an apply error");
    assert_eq!(payload.conflicted_files, vec!["alpha.txt", "beta.txt"]);
}

/// `resolve` answers with the workspace-sync payload under its own kind, reporting what is still
/// conflicted: resolving `alpha.txt` leaves `beta.txt`.
#[test]
fn test_resolve_fixture() {
    let envelope = check_success_fixture::<WorkspaceSyncJson>("resolve.json", "resolve");
    let payload = &envelope.message.payload;
    assert_eq!(payload.target_revision, "main.1.2");
    assert_eq!(payload.conflicted_files, vec!["beta.txt"]);
}

/// Captured by killing a `fxv goto` partway and running `fxv status` afterwards. This is the only
/// fixture with `error_data`, and the only one at exit code 98.
#[test]
fn test_interrupted_sync_error_fixture() {
    let text = read_fixture("interrupted_sync_error.json");

    let raw: Value = serde_json::from_str(&text).unwrap();
    let validator = build_envelope_validator();
    let errors: Vec<_> = validator.iter_errors(&raw).collect();
    assert!(
        errors.is_empty(),
        "interrupted_sync_error.json fails schema validation: {errors:?}"
    );

    let ParsedOutput::Error(envelope) = parse_output::<StatusJson>(&text).unwrap() else {
        panic!("the interrupted-sync fixture should parse as an error");
    };
    assert_eq!(envelope.message.payload.exit_code(), ExitCode::InterruptedSync);

    let data = envelope
        .message
        .payload
        .error_data
        .as_ref()
        .expect("exit 98 includes interrupted-sync detail");
    assert_eq!(data.kind, "interrupted-sync");

    // The nested payload is an opaque object in the error schema, so validate it separately, the
    // way a consumer dispatches on `kind` first.
    let detail_validator = jsonschema::validator_for(&load_schema("interrupted_sync.schema.json")).unwrap();
    let detail_errors: Vec<_> = detail_validator.iter_errors(&data.payload).collect();
    assert!(
        detail_errors.is_empty(),
        "error_data payload fails the interrupted-sync schema: {detail_errors:?}"
    );

    let InterruptedSyncJson::Recoverable {
        operation,
        completed_entries,
        remaining_entries,
        total_entries,
        sampled_unfinished_paths,
        source_revision,
        ..
    } = serde_json::from_value(data.payload.clone()).unwrap()
    else {
        panic!("the interrupted run left a readable journal");
    };
    assert_eq!(operation, InterruptedOperation::Goto);
    assert_eq!(completed_entries + remaining_entries, total_entries);
    assert_eq!(sampled_unfinished_paths.len(), remaining_entries);
    assert_eq!(source_revision.as_deref(), Some("main.-.6"));

    // Known upstream drift: these keep the platform separator where every other payload
    // normalizes to '/'. Captured on Windows, recording what the CLI wrote.
    assert!(sampled_unfinished_paths.iter().all(|path| path.contains(r"\")));
}

#[test]
fn test_error_fixture() {
    // Captured by running `fxv status` outside any workspace.
    let text = read_fixture("error.json");

    let raw: Value = serde_json::from_str(&text).unwrap();
    let validator = build_envelope_validator();
    let errors: Vec<_> = validator.iter_errors(&raw).collect();
    assert!(errors.is_empty(), "error.json fails schema validation: {errors:?}");

    // The expected payload type is irrelevant: an error envelope always parses as Error.
    match parse_output::<StatusJson>(&text).unwrap() {
        ParsedOutput::Error(envelope) => {
            assert_eq!(envelope.message.kind, "error");
            assert_eq!(envelope.message.payload.exit_code(), ExitCode::GeneralError);
            assert!(envelope.message.payload.message.contains("No workspace found"));
        }
        ParsedOutput::Success(_) => panic!("error fixture parsed as success"),
    }
}
