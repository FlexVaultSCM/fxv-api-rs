//! Parses envelopes captured from a real `fxv` binary (see `tests/fixtures/`) with the typed
//! structs, and re-serializes them to prove the field/omission rules match the CLI byte-for-byte
//! (as JSON values). Fixtures were captured from a local-only workspace; regenerate them with a
//! current `fxv` build if the wire format changes.

// == Std
use std::path::Path;

// == Internal crates
use fxv_api::v1::{
    change_info::ChangeInfoOutputJson,
    envelope::{ExitCode, OutputEnvelope, ParsedOutput, parse_output},
    history::HistoryOutputJson,
    init::InitJson,
    logout::LogoutJson,
    status::{HeadCommitJson, StatusJson},
};

// == External crates
use jsonschema::{Resource, Validator};
use serde_json::Value;

fn read_fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read fixture {name}: {e}"))
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
        ("urn:fxv:schema:login:v1", "login.schema.json"),
        ("urn:fxv:schema:logout:v1", "logout.schema.json"),
        ("urn:fxv:schema:status:v1", "status.schema.json"),
        ("urn:fxv:schema:user:v1", "user.schema.json"),
        ("urn:fxv:schema:workspace-sync:v1", "workspace_sync.schema.json"),
    ];
    let load = |file_name: &str| -> Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas").join(file_name);
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
    };
    let mut options = jsonschema::options();
    for (urn, file_name) in SCHEMA_RESOURCES {
        options.with_resource(*urn, Resource::from_contents(load(file_name)).expect(file_name));
    }
    options
        .build(&load("envelope.schema.json"))
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
