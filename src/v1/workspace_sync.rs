//! Shared payload for the workspace-sync family: `fxv sync`, `fxv goto`, `fxv resolve`, and
//! `fxv revert` all emit this shape under their own `message.kind` (`sync` / `goto` / `resolve` /
//! `revert`). Matches `schemas/workspace_sync.schema.json` (`urn:fxv:schema:workspace-sync:v1`).

// == Internal crates
use crate::v1::common::{CommitRefJson, FileStatusJson};

// == External crates
use serde::{Deserialize, Serialize};

/// JSON payload for a completed workspace-sync operation.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkspaceSyncJson {
    /// Spec string of the revision the workspace ended at (e.g. `main.4` or `main.4.2`).
    pub target_revision: String,
    /// Snapshots created by this operation, in creation order (pre-sync snapshot before preserved).
    /// Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub created_revisions: Vec<CommitRefJson>,
    pub files_updated_count: usize,
    pub error_count: usize,
    pub files_updated: Vec<FileStatusJson>,
    /// Unresolved conflicts in the target tree, which block publishing. Omitted when there are
    /// none. Paths are workspace-relative and `/`-separated.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicted_files: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::{
        common::{ChangeKind, test_support::*},
        envelope::test_support::*,
    };

    #[test]
    fn test_workspace_sync_round_trip_and_schema() {
        let payload = WorkspaceSyncJson {
            target_revision: "main.6".to_string(),
            created_revisions: vec![sample_draft_commit_ref("main", Some(4), 3)],
            files_updated_count: 1,
            error_count: 0,
            files_updated: vec![FileStatusJson {
                path: "maps/level1.umap".to_string(),
                unpublished_state: None,
                workspace_state: Some(ChangeKind::Modified),
                conflict_state: None,
                size: Some(4096),
            }],
            conflicted_files: vec!["maps/level2.umap".to_string()],
        };
        // Every sync-family kind emits the same payload shape; validate each against the schema.
        for kind in ["sync", "goto", "resolve", "revert"] {
            let json = assert_payload_validates(kind, &payload);
            let parsed: WorkspaceSyncJson = serde_json::from_value(json["message"]["payload"].clone()).unwrap();
            assert_eq!(parsed, payload);
        }
    }

    #[test]
    fn test_empty_lists_are_omitted_and_default_on_parse() {
        let payload = WorkspaceSyncJson {
            target_revision: "main.2".to_string(),
            created_revisions: vec![],
            files_updated_count: 0,
            error_count: 0,
            files_updated: vec![],
            conflicted_files: vec![],
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert!(json.get("created_revisions").is_none());
        assert!(json.get("conflicted_files").is_none());

        // The omitted fields must come back as empty vecs, not a parse error.
        let parsed: WorkspaceSyncJson = serde_json::from_value(json).unwrap();
        assert_eq!(parsed, payload);
    }
}
