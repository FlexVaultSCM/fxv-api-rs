//! Payload for `fxv changeinfo` (`message.kind == "changeinfo"`). Matches
//! `schemas/changeinfo.schema.json` (`urn:fxv:schema:changeinfo:v1`).

// == Internal crates
use crate::v1::common::CommitRefJson;

// == External crates
use serde::{Deserialize, Serialize};

/// JSON payload for `fxv changeinfo`: one revision's commit reference plus its file changes.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ChangeInfoOutputJson {
    #[serde(flatten)]
    pub commit_ref: CommitRefJson,
    pub summary: ChangeSummaryJson,
    pub changes: Vec<FileChangeJson>,
}

/// Counts of the file changes in the revision (directories excluded).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ChangeSummaryJson {
    pub total_changed: usize,
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
}

/// A single file change within the revision.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FileChangeJson {
    /// Workspace-relative path, always `/`-separated regardless of platform.
    pub path: String,
    pub action: FileChangeAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_hash: Option<String>,
}

/// What happened to the file in this revision. The CLI serializes this from a `&'static str`;
/// here it is a real enum so it can round-trip.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeAction {
    Added,
    Modified,
    Deleted,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::{common::test_support::*, envelope::test_support::*};

    #[test]
    fn test_changeinfo_round_trip_and_schema() {
        let payload = ChangeInfoOutputJson {
            commit_ref: sample_draft_commit_ref("main", Some(4), 1),
            summary: ChangeSummaryJson {
                total_changed: 2,
                added: 1,
                modified: 0,
                deleted: 1,
            },
            changes: vec![
                FileChangeJson {
                    path: "shaders/water.hlsl".to_string(),
                    action: FileChangeAction::Added,
                    size: Some(2048),
                    old_hash: None,
                    new_hash: Some("ab".repeat(32)),
                },
                FileChangeJson {
                    path: "shaders/old.hlsl".to_string(),
                    action: FileChangeAction::Deleted,
                    size: None,
                    old_hash: Some("cd".repeat(32)),
                    new_hash: None,
                },
            ],
        };
        let json = assert_payload_validates("changeinfo", &payload);
        let parsed: ChangeInfoOutputJson = serde_json::from_value(json["message"]["payload"].clone()).unwrap();
        assert_eq!(parsed, payload);
        assert_eq!(json["message"]["payload"]["changes"][0]["action"], "added");
    }
}
