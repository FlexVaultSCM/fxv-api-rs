//! Parses envelopes captured from a real `fxv` binary (see `tests/fixtures/`) with the typed
//! structs, and re-serializes them to prove the field/omission rules match the CLI byte-for-byte
//! (as JSON values). Fixtures were captured from a local-only workspace; regenerate them with a
//! current `fxv` build if the wire format changes.
//!
//! Payloads are verbatim. The `program` block's `executable`, `arguments[0]`, and
//! `working_directory` are rewritten to a neutral path when a fixture is added, because the CLI
//! reports the real invocation and there is no reason to publish whoever captured it.

// == Std
use std::path::Path;

// == Internal crates
use fxv_api::v1::{
    change_info::ChangeInfoOutputJson,
    envelope::{ExitCode, OutputEnvelope, ParsedOutput, parse_output},
    history::HistoryOutputJson,
    init::InitJson,
    interrupted_sync::{InterruptedOperation, InterruptedSyncJson},
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
        ("urn:fxv:schema:status:v1", "status.schema.json"),
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
    envelope
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
    assert_eq!(status.file_change_counts.total, status.files.len());
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

    // The error schema types the nested payload as an opaque object, so its own schema has to be
    // applied separately - the same way a consumer dispatches on `kind` before reading it.
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

    // Known upstream drift: these paths keep the platform separator, where every other payload
    // normalizes to '/'. The fixture was captured on Windows and records what the CLI wrote.
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
