//! The outer envelope every `fxv --format json` invocation prints to stdout, success or failure.
//! Matches `schemas/envelope.schema.json` (`urn:fxv:schema:envelope:v1`).

// == Std
use std::{fmt, str::FromStr};

// == External crates
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};

/// One complete JSON message from the CLI: metadata about the producing process plus the
/// command-specific payload. Errors use the same envelope with a `message.kind` of `"error"`
/// and an [`ErrorJson`] payload, also on stdout, so consumers only ever parse one stream.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OutputEnvelope<T> {
    pub program: ProgramMetadata,
    pub message: MessageEnvelope<T>,
}

/// Details of the `fxv` process that produced a message. `arguments` is the full argv with
/// sensitive flag values (S3 credentials) redacted to `"***"` by the CLI before emission.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ProgramMetadata {
    pub name: String,
    pub version: String,
    pub executable: String,
    pub arguments: Vec<String>,
    /// RFC 3339 timestamp of the invocation.
    pub invoked_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
}

/// The message half of the envelope. `kind` is the dash-joined CLI command path (`status`,
/// `user-add`, ...), `error` for failures, or a `-progress`-suffixed kind for interim JSONL
/// updates (reserved; the CLI does not emit these yet). `update_frequency_seconds` and
/// `sequence` are only present on progress messages.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MessageEnvelope<T> {
    pub kind: String,
    /// The payload schema's version on this `kind`'s own timeline. Every kind starts at `1.0`; the
    /// minor moves for an additive change, the major for anything a consumer could break on.
    pub version: MessageVersion,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_frequency_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u64>,
    pub payload: T,
}

/// A message's `major.minor` payload-schema version, versioned per `kind` rather than globally.
/// Mirrors the CLI's `MessageVersion`: on the wire it is the string `"1.0"`, not a number, so `1.10`
/// cannot be confused with `1.1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageVersion {
    pub major: u32,
    pub minor: u32,
}

impl MessageVersion {
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// Whether a consumer written against `expected` can read a payload of this version: the major
    /// must match exactly, and the payload must carry at least the minor the consumer expects.
    pub fn is_compatible_with(&self, expected: MessageVersion) -> bool {
        self.major == expected.major && self.minor >= expected.minor
    }
}

impl fmt::Display for MessageVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// A `version` string the CLI could not have produced (the envelope schema pins it to `major.minor`).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("Invalid message version '{0}', expected 'major.minor'")]
pub struct MessageVersionParseError(String);

impl FromStr for MessageVersion {
    type Err = MessageVersionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || MessageVersionParseError(s.to_string());
        let (major, minor) = s.split_once('.').ok_or_else(invalid)?;
        Ok(Self {
            major: major.parse().map_err(|_| invalid())?,
            minor: minor.parse().map_err(|_| invalid())?,
        })
    }
}

impl Serialize for MessageVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for MessageVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

/// JSON payload for a failed command (`message.kind == "error"`). Matches
/// `schemas/error.schema.json`. `backtrace` is present only when the CLI actually captured one
/// (`RUST_BACKTRACE` set).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ErrorJson {
    pub message: String,
    pub exit_code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backtrace: Option<String>,
    /// Machine-readable detail for the failures that have any, so a consumer can act without
    /// parsing `message`. Absent for most errors; `interrupted-sync` (exit code 98) is the only
    /// kind the CLI emits today.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_data: Option<ErrorData>,
}

/// Detail riding on an error. Mirrors [`MessageEnvelope`]'s kind/version/payload triple, so a
/// consumer dispatches on it the same way, and each detail is versioned independently of the error
/// envelope around it. `payload` stays a raw value, since `kind` decides how to read it, and the
/// caller deserializes it once it has matched (see [`crate::v1::interrupted_sync`]).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ErrorData {
    pub kind: String,
    pub version: MessageVersion,
    pub payload: serde_json::Value,
}

/// Process exit codes the CLI commits to. Mirrors `fxv_cli::ExitCode`; codes other than these
/// map to [`ExitCode::Other`] so new codes are not a breaking change for consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    Ok,
    GeneralError,
    /// The command rejected its arguments. Code 2, the same code clap exits with for its own parse errors.
    UsageError,
    /// `fxv upgrade --check` succeeded and found an integration compatibility conflict. Code 3.
    UpgradeCheckConflictsFound,
    /// `fxv doctor` completed and found at least one failing check. Code 4.
    DoctorIssuesFound,
    /// The command succeeded but printed warnings saying what it could not do. Code 67.
    SucceededWithWarnings,
    /// The broker refused this `fxv` as older than the oldest version it serves, so the user should install a newer
    /// release rather than retry. Code 94, and the error includes `broker-error` detail in `error_data`.
    ClientUpgradeRequired,
    /// A brokered operation needs the user to sign in. Code 95.
    AuthRequired,
    /// The repository is in an older format and must be upgraded with `fxv repo upgrade`. Code 96.
    RepositoryUpgradeRequired,
    /// The remote answered and has no repository, so one can be created with `fxv repo new`. Code 97.
    RemoteRepositoryNotFound,
    /// A previous sync was interrupted and the workspace must be recovered with `fxv resume`
    /// before anything else can run. Reserved code 98, and the error includes `interrupted-sync`
    /// detail in `error_data`.
    InterruptedSync,
    /// The workspace was locked by another fxv process. Reserved code 99.
    WorkspaceLocked,
    Other(i32),
}

const EXIT_CODES: &[(ExitCode, i32)] = &[
    (ExitCode::Ok, 0),
    (ExitCode::GeneralError, 1),
    (ExitCode::UsageError, 2),
    (ExitCode::UpgradeCheckConflictsFound, 3),
    (ExitCode::DoctorIssuesFound, 4),
    (ExitCode::SucceededWithWarnings, 67),
    (ExitCode::ClientUpgradeRequired, 94),
    (ExitCode::AuthRequired, 95),
    (ExitCode::RepositoryUpgradeRequired, 96),
    (ExitCode::RemoteRepositoryNotFound, 97),
    (ExitCode::InterruptedSync, 98),
    (ExitCode::WorkspaceLocked, 99),
];

impl From<i32> for ExitCode {
    fn from(code: i32) -> Self {
        EXIT_CODES
            .iter()
            .find(|(_, value)| *value == code)
            .map_or(ExitCode::Other(code), |(exit_code, _)| *exit_code)
    }
}

impl From<ExitCode> for i32 {
    fn from(code: ExitCode) -> Self {
        match code {
            ExitCode::Other(other) => other,
            known => EXIT_CODES
                .iter()
                .find(|(exit_code, _)| *exit_code == known)
                .map(|(_, value)| *value)
                .expect("every named exit code has a value"),
        }
    }
}

impl ErrorJson {
    pub fn exit_code(&self) -> ExitCode {
        self.exit_code.into()
    }
}

/// The decoded output of one CLI invocation: the expected payload, or the CLI's own error report.
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedOutput<T> {
    Success(OutputEnvelope<T>),
    Error(OutputEnvelope<ErrorJson>),
}

/// Parses one complete stdout envelope, discriminating success from error by `message.kind`.
/// Returns `Err` only when the text is not a well-formed envelope at all (or the payload does not
/// match `T`) — a CLI-reported failure is the `Ok(ParsedOutput::Error(..))` case.
pub fn parse_output<T: DeserializeOwned>(json: &str) -> Result<ParsedOutput<T>, serde_json::Error> {
    let raw: OutputEnvelope<serde_json::Value> = serde_json::from_str(json)?;
    let OutputEnvelope { program, message } = raw;
    if message.kind == "error" {
        Ok(ParsedOutput::Error(OutputEnvelope {
            program,
            message: MessageEnvelope {
                payload: serde_json::from_value(message.payload)?,
                kind: message.kind,
                version: message.version,
                update_frequency_seconds: message.update_frequency_seconds,
                sequence: message.sequence,
            },
        }))
    } else {
        Ok(ParsedOutput::Success(OutputEnvelope {
            program,
            message: MessageEnvelope {
                payload: serde_json::from_value(message.payload)?,
                kind: message.kind,
                version: message.version,
                update_frequency_seconds: message.update_frequency_seconds,
                sequence: message.sequence,
            },
        }))
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    // == Std
    use std::path::Path;

    // == External crates
    use jsonschema::{Resource, Validator};
    use serde_json::Value;

    /// Every payload schema in `schemas/`, keyed by its `$id`, so `envelope.schema.json`'s `$ref`s resolve.
    fn schema_resources() -> Vec<(String, Value)> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas");
        let mut resources = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("read schemas dir") {
            let file_name = entry
                .expect("read schemas entry")
                .file_name()
                .into_string()
                .expect("utf-8 file name");
            if !file_name.ends_with(".schema.json") || file_name == "envelope.schema.json" {
                continue;
            }
            let schema = load_schema(&file_name);
            let urn = schema["$id"]
                .as_str()
                .unwrap_or_else(|| panic!("{file_name} has no $id"))
                .to_string();
            resources.push((urn, schema));
        }
        resources
    }

    fn load_schema(file_name: &str) -> Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas").join(file_name);
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {file_name}: {e}")))
            .unwrap_or_else(|e| panic!("parse {file_name}: {e}"))
    }

    /// Compiles `envelope.schema.json` with every payload schema registered, so a full envelope
    /// (any command kind) can be validated. Mirrors the validator the CLI's own tests build.
    pub(crate) fn build_envelope_validator() -> Validator {
        let mut options = jsonschema::options();
        for (urn, schema) in schema_resources() {
            let resource = Resource::from_contents(schema).expect(&urn);
            options.with_resource(&urn, resource);
        }
        options
            .build(&load_schema("envelope.schema.json"))
            .expect("Failed to compile envelope schema")
    }

    /// Validates a value against one payload schema on its own, for payloads that never appear as
    /// a top-level message: `interrupted-sync` only ever rides inside an error's `error_data`,
    /// where the error schema types it as an opaque object.
    pub(crate) fn assert_value_validates<T: serde::Serialize>(urn: &str, payload: &T) {
        let schema = schema_resources()
            .into_iter()
            .find(|(registered, _)| registered == urn)
            .map(|(_, schema)| schema)
            .unwrap_or_else(|| panic!("no schema registered for {urn}"));
        let validator = jsonschema::validator_for(&schema).expect(urn);
        let json = serde_json::to_value(payload).unwrap();
        let errors: Vec<_> = validator.iter_errors(&json).collect();
        assert!(errors.is_empty(), "Schema validation errors for {urn}: {errors:?}");
    }

    /// Serializes `payload` under a complete-message envelope of `kind` and asserts it validates.
    pub(crate) fn assert_payload_validates<T: serde::Serialize>(kind: &str, payload: &T) -> Value {
        let envelope = super::OutputEnvelope {
            program: sample_program_metadata(),
            message: super::MessageEnvelope {
                kind: kind.to_string(),
                version: super::MessageVersion::new(1, 0),
                update_frequency_seconds: None,
                sequence: None,
                payload,
            },
        };
        let json = serde_json::to_value(&envelope).unwrap();
        let validator = build_envelope_validator();
        let errors: Vec<_> = validator.iter_errors(&json).collect();
        assert!(
            errors.is_empty(),
            "Schema validation errors for kind {kind}: {errors:?}"
        );
        json
    }

    pub(crate) fn sample_program_metadata() -> super::ProgramMetadata {
        super::ProgramMetadata {
            name: "fxv".to_string(),
            version: "0.0.0-test".to_string(),
            executable: "/usr/bin/fxv".to_string(),
            arguments: vec![
                "fxv".to_string(),
                "status".to_string(),
                "--format".to_string(),
                "json".to_string(),
            ],
            invoked_at: "2026-01-01T00:00:00Z".to_string(),
            hostname: None,
            pid: Some(4242),
            working_directory: Some("/work".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{test_support::*, *};

    #[test]
    fn test_exit_codes_round_trip() {
        for (exit_code, value) in EXIT_CODES {
            assert_eq!(ExitCode::from(*value), *exit_code);
            assert_eq!(i32::from(*exit_code), *value);
        }
        assert_eq!(ExitCode::from(5), ExitCode::Other(5));
        assert_eq!(i32::from(ExitCode::Other(5)), 5);
    }

    #[test]
    fn test_error_envelope_round_trip_and_schema() {
        let payload = ErrorJson {
            message: "Workspace is locked by another process.".to_string(),
            exit_code: ExitCode::WorkspaceLocked.into(),
            backtrace: Some("0: some::frame".to_string()),
            error_data: None,
        };
        let json = assert_payload_validates("error", &payload);
        assert_eq!(json["message"]["payload"]["exit_code"], 99);

        let parsed: ErrorJson = serde_json::from_value(json["message"]["payload"].clone()).unwrap();
        assert_eq!(parsed, payload);
        assert_eq!(parsed.exit_code(), ExitCode::WorkspaceLocked);
    }

    #[test]
    fn test_missing_backtrace_is_omitted_not_null() {
        let payload = ErrorJson {
            message: "boom".to_string(),
            exit_code: ExitCode::GeneralError.into(),
            backtrace: None,
            error_data: None,
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert!(json.get("backtrace").is_none());
    }

    #[test]
    fn test_parse_output_discriminates_error_kind() {
        let envelope = OutputEnvelope {
            program: sample_program_metadata(),
            message: MessageEnvelope {
                kind: "error".to_string(),
                version: MessageVersion::new(1, 0),
                update_frequency_seconds: None,
                sequence: None,
                payload: ErrorJson {
                    message: "no workspace".to_string(),
                    exit_code: 1,
                    backtrace: None,
                    error_data: None,
                },
            },
        };
        let text = serde_json::to_string(&envelope).unwrap();

        // Even when the caller expected a login payload, an error envelope parses as Error.
        match parse_output::<crate::v1::login::LoginJson>(&text).unwrap() {
            ParsedOutput::Error(e) => {
                assert_eq!(e.message.payload.message, "no workspace");
                assert_eq!(e.message.payload.exit_code(), ExitCode::GeneralError);
            }
            ParsedOutput::Success(_) => panic!("error envelope parsed as success"),
        }
    }

    #[test]
    fn test_parse_output_success() {
        let envelope = OutputEnvelope {
            program: sample_program_metadata(),
            message: MessageEnvelope {
                kind: "login".to_string(),
                version: MessageVersion::new(1, 0),
                update_frequency_seconds: None,
                sequence: None,
                payload: crate::v1::login::LoginJson {
                    username: "alice".to_string(),
                },
            },
        };
        let text = serde_json::to_string(&envelope).unwrap();

        match parse_output::<crate::v1::login::LoginJson>(&text).unwrap() {
            ParsedOutput::Success(s) => assert_eq!(s.message.payload.username, "alice"),
            ParsedOutput::Error(_) => panic!("success envelope parsed as error"),
        }
    }

    #[test]
    fn test_exit_code_mapping() {
        assert_eq!(ExitCode::from(0), ExitCode::Ok);
        assert_eq!(ExitCode::from(1), ExitCode::GeneralError);
        assert_eq!(ExitCode::from(99), ExitCode::WorkspaceLocked);
        assert_eq!(ExitCode::from(42), ExitCode::Other(42));
        assert_eq!(i32::from(ExitCode::WorkspaceLocked), 99);
    }
}
