//! Payload for `fxv status` (`message.kind == "status"`). Matches `schemas/status.schema.json`
//! (`urn:fxv:schema:status:v1`).

// == Internal crates
use crate::v1::common::{CommitRefJson, FileStatusJson};

// == External crates
use serde::{Deserialize, Serialize};

/// JSON payload for `fxv status`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct StatusJson {
    pub current_branch: String,
    pub head_commit: HeadCommitJson,
    /// Present only for parented drafts (absent for an empty branch or unparented draft).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_status: Option<SyncStatusJson>,
    pub files: Vec<FileStatusJson>,
    pub file_change_counts: FileChangeCountsJson,
}

/// The workspace head commit, tagged by `state`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum HeadCommitJson {
    EmptyBranch {
        branch: String,
    },
    UnparentedDraft {
        local_snapshot: Box<CommitRefJson>,
    },
    ParentedDraft {
        local_snapshot: Box<CommitRefJson>,
        published_head: Box<CommitRefJson>,
    },
}

/// How the workspace's published head compares to the last synced revision (parented drafts only).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SyncStatusJson {
    pub up_to_date: bool,
    pub revisions_behind: u64,
    pub published_head_revision: u64,
    pub synced_revision: Option<u64>,
}

/// Counts summarizing the changed files.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FileChangeCountsJson {
    pub total: usize,
    pub unpublished: usize,
    pub workspace_need_snapshot: usize,
}

impl StatusJson {
    /// Files carrying an unpublished (draft) change.
    pub fn unpublished_files(&self) -> impl Iterator<Item = &FileStatusJson> {
        self.files.iter().filter(|f| f.unpublished_state.is_some())
    }

    /// Files with working-tree changes not yet captured in a snapshot.
    pub fn workspace_files(&self) -> impl Iterator<Item = &FileStatusJson> {
        self.files.iter().filter(|f| f.workspace_state.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::{
        common::{ChangeKind, test_support::*},
        envelope::test_support::*,
    };

    fn sample_status() -> StatusJson {
        StatusJson {
            current_branch: "main".to_string(),
            head_commit: HeadCommitJson::ParentedDraft {
                local_snapshot: Box::new(sample_draft_commit_ref("main", Some(4), 2)),
                published_head: Box::new(sample_commit_ref("main", 6)),
            },
            sync_status: Some(SyncStatusJson {
                up_to_date: false,
                revisions_behind: 2,
                published_head_revision: 6,
                synced_revision: Some(4),
            }),
            files: vec![
                FileStatusJson {
                    path: "assets/logo.png".to_string(),
                    unpublished_state: Some(ChangeKind::Added),
                    workspace_state: None,
                    conflict_state: None,
                    size: Some(1024),
                },
                FileStatusJson {
                    path: "src/main.rs".to_string(),
                    unpublished_state: None,
                    workspace_state: Some(ChangeKind::MaybeChanged),
                    conflict_state: None,
                    size: None,
                },
            ],
            file_change_counts: FileChangeCountsJson {
                total: 2,
                unpublished: 1,
                workspace_need_snapshot: 1,
            },
        }
    }

    #[test]
    fn test_status_round_trip_and_schema() {
        let payload = sample_status();
        let json = assert_payload_validates("status", &payload);
        let parsed: StatusJson = serde_json::from_value(json["message"]["payload"].clone()).unwrap();
        assert_eq!(parsed, payload);
    }

    #[test]
    fn test_empty_branch_head_validates() {
        let payload = StatusJson {
            current_branch: "main".to_string(),
            head_commit: HeadCommitJson::EmptyBranch {
                branch: "main".to_string(),
            },
            sync_status: None,
            files: vec![],
            file_change_counts: FileChangeCountsJson {
                total: 0,
                unpublished: 0,
                workspace_need_snapshot: 0,
            },
        };
        let json = assert_payload_validates("status", &payload);
        assert_eq!(json["message"]["payload"]["head_commit"]["state"], "empty_branch");
    }

    #[test]
    fn test_file_axis_helpers() {
        let status = sample_status();
        assert_eq!(status.unpublished_files().count(), 1);
        assert_eq!(status.workspace_files().count(), 1);
    }
}
