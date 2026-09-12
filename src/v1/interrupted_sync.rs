//! Detail payload for a sync that was interrupted partway (`error_data.kind ==
//! "interrupted-sync"`). Matches `schemas/interrupted_sync.schema.json`
//! (`urn:fxv:schema:interrupted-sync:v1`).
//!
//! Never a top-level message. It rides in an error envelope's `error_data` alongside exit code 98,
//! when a command refuses to run because the workspace is mid-operation. `fxv resume` recovers,
//! `fxv status` reports, so there is no "nothing was interrupted" case here.

// == External crates
use serde::{Deserialize, Serialize};

/// A sync, goto, revert, or resolve that stopped partway, tagged by `state`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum InterruptedSyncJson {
    /// The journal was readable, so the run can be finished with `fxv resume --continue` or undone
    /// with `fxv resume --rollback`.
    Recoverable {
        operation: InterruptedOperation,
        /// A ready-to-display sentence describing what the run was doing, phrased per operation,
        /// e.g. "Was syncing from main.11 to main.12."
        summary: String,
        /// Revision spec the run was moving to, and where a continue finishes.
        target_revision: String,
        /// Revision spec the workspace is still recorded as synced to, and where a rollback
        /// returns to. Omitted when the run was a checkout into a workspace that had never synced.
        #[serde(skip_serializing_if = "Option::is_none")]
        source_revision: Option<String>,
        total_entries: usize,
        completed_entries: usize,
        /// Entries whose locally-modified file was moved aside rather than overwritten.
        preserved_entries: usize,
        failed_entries: usize,
        /// Entries that never reached a terminal state, and which a continue re-drives.
        remaining_entries: usize,
        /// A capped sample of the unfinished paths; `remaining_entries` is the true count.
        sampled_unfinished_paths: Vec<String>,
    },
    /// The journal survived but could not be parsed, so neither recovery operation can act on it.
    Unreadable { journal_path: String, reason: String },
}

/// The command that was interrupted. A real enum here, where the CLI writes a `&'static str`.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InterruptedOperation {
    Sync,
    Goto,
    Revert,
    Resolve,
    Rollback,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::envelope::{ErrorData, ErrorJson, ExitCode, MessageVersion, test_support::*};

    fn sample_recoverable() -> InterruptedSyncJson {
        InterruptedSyncJson::Recoverable {
            operation: InterruptedOperation::Goto,
            summary: "Was going from main.11 to main.12.".to_string(),
            target_revision: "main.12".to_string(),
            source_revision: Some("main.11".to_string()),
            total_entries: 10,
            completed_entries: 6,
            preserved_entries: 1,
            failed_entries: 0,
            remaining_entries: 3,
            sampled_unfinished_paths: vec!["a.txt".to_string(), "dir/b.txt".to_string()],
        }
    }

    #[test]
    fn test_recoverable_round_trip_and_schema() {
        let payload = sample_recoverable();
        assert_value_validates("urn:fxv:schema:interrupted-sync:v1", &payload);
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["state"], "recoverable");
        assert_eq!(json["operation"], "goto");
        let parsed: InterruptedSyncJson = serde_json::from_value(json).unwrap();
        assert_eq!(parsed, payload);
    }

    /// A checkout into a never-synced workspace has nowhere to roll back to, so the field is
    /// omitted rather than serialized as null.
    #[test]
    fn test_missing_source_revision_is_omitted() {
        let InterruptedSyncJson::Recoverable {
            operation,
            summary,
            target_revision,
            total_entries,
            completed_entries,
            preserved_entries,
            failed_entries,
            remaining_entries,
            sampled_unfinished_paths,
            ..
        } = sample_recoverable()
        else {
            unreachable!("the sample is recoverable");
        };
        let payload = InterruptedSyncJson::Recoverable {
            operation,
            summary,
            target_revision,
            source_revision: None,
            total_entries,
            completed_entries,
            preserved_entries,
            failed_entries,
            remaining_entries,
            sampled_unfinished_paths,
        };

        let json = serde_json::to_value(&payload).unwrap();
        assert!(json.get("source_revision").is_none());
        assert_value_validates("urn:fxv:schema:interrupted-sync:v1", &payload);
    }

    #[test]
    fn test_unreadable_round_trip_and_schema() {
        let payload = InterruptedSyncJson::Unreadable {
            journal_path: "/ws/.fxv_workspace/state/sync_progress.rkyv".to_string(),
            reason: "Sync journal magic bytes do not match".to_string(),
        };
        assert_value_validates("urn:fxv:schema:interrupted-sync:v1", &payload);
        let parsed: InterruptedSyncJson = serde_json::from_value(serde_json::to_value(&payload).unwrap()).unwrap();
        assert_eq!(parsed, payload);
    }

    /// How a consumer actually meets this payload: exit code 98, an error envelope, and the detail
    /// nested in `error_data` under its own kind and version.
    #[test]
    fn test_rides_in_an_error_envelope() {
        let detail = sample_recoverable();
        let payload = ErrorJson {
            message: "A previous `fxv goto` was interrupted.".to_string(),
            exit_code: ExitCode::InterruptedSync.into(),
            backtrace: None,
            error_data: Some(ErrorData {
                kind: "interrupted-sync".to_string(),
                version: MessageVersion::new(1, 0),
                payload: serde_json::to_value(&detail).unwrap(),
            }),
        };

        let json = assert_payload_validates("error", &payload);
        assert_eq!(json["message"]["payload"]["exit_code"], 98);

        let parsed: ErrorJson = serde_json::from_value(json["message"]["payload"].clone()).unwrap();
        assert_eq!(parsed.exit_code(), ExitCode::InterruptedSync);
        let data = parsed.error_data.expect("error_data should survive the round trip");
        assert_eq!(data.kind, "interrupted-sync");
        assert_eq!(data.version, MessageVersion::new(1, 0));
        assert_eq!(
            serde_json::from_value::<InterruptedSyncJson>(data.payload).unwrap(),
            detail
        );
    }
}
