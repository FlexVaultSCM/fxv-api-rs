//! Types shared by several command payloads, matching the `$defs` in `schemas/common.schema.json`
//! (`urn:fxv:schema:common:v1`).

// == External crates
use serde::{Deserialize, Serialize};

/// A commit reference: where the commit sits on its branch, plus everything about the commit itself
/// a consumer normally wants (description, timestamp, resolved author, import provenance). Shared by
/// every command that surfaces a commit in its JSON output (`status`, `history`, `changeinfo`,
/// `sync`/`goto`/`resolve`/`revert`), corresponding to the `commitRef` definition in
/// `common.schema.json`. `commit_hash` is optional so `history` (which has never included it) can
/// keep omitting it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CommitRefJson {
    pub commit: CommitInfoJson,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
    /// Commit description, omitted when the commit was made without one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Commit timestamp in milliseconds since the epoch (UTC). For an imported commit this is the
    /// original authoring time, not when the import ran.
    pub timestamp_millis_since_epoch_utc: i64,
    pub author_id: String,
    pub author_display_name: String,
    pub author_details: AuthorDisplayInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import_info: Option<ImportInfoDisplay>,
}

/// Identifies a revision on a branch: `main.4` published, `main.4.2` draft, `main.-.2` unparented
/// draft (in spec-string terms).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CommitInfoJson {
    pub branch: String,
    /// Published revision number. For a draft this is the published parent it is based on; `None`
    /// (omitted) when the draft has no published parent, so an unparented draft is distinguishable
    /// from one parented on published revision 0 (spec "main.-.N" vs "main.0.N").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<u64>,
    #[serde(rename = "type")]
    pub commit_type: CommitType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft_revision: Option<u64>,
}

impl CommitInfoJson {
    /// Formats the revision-ID spec string, mirroring `fxv_repo`'s `CommitInfo::to_spec_string`:
    /// `main.4` published, `main.4.2` draft, `main.-.2` unparented draft.
    pub fn to_spec_string(&self) -> String {
        match (self.revision, self.draft_revision) {
            (Some(published), Some(draft)) => format!("{}.{}.{}", self.branch, published, draft),
            (Some(published), None) => format!("{}.{}", self.branch, published),
            (None, Some(draft)) => format!("{}.-.{}", self.branch, draft),
            // The CLI never emits a commit with neither revision; fall back to the bare branch.
            (None, None) => self.branch.clone(),
        }
    }
}

/// Whether a commit is published or a local draft. The CLI serializes this from a `&'static str`;
/// here it is a real enum so it can round-trip.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommitType {
    Published,
    Draft,
}

/// The resolved author of a commit, tagged by `type`. Mirrors `fxv_cli_ux`'s `AuthorDisplayInfo`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum AuthorDisplayInfo {
    Local,
    Service,
    FxvUser {
        id: u32,
        username: String,
        display_name: String,
    },
    GitUser {
        name: String,
        email: String,
    },
    P4User {
        username: String,
        email: String,
        display_name: String,
    },
    Error {
        message: String,
    },
}

impl AuthorDisplayInfo {
    pub fn display_name(&self) -> String {
        match self {
            AuthorDisplayInfo::Local => "(local user)".to_string(),
            AuthorDisplayInfo::Service => "(service user)".to_string(),
            AuthorDisplayInfo::FxvUser { display_name, .. } => display_name.clone(),
            AuthorDisplayInfo::GitUser { name, .. } => name.clone(),
            AuthorDisplayInfo::P4User { display_name, .. } => display_name.clone(),
            AuthorDisplayInfo::Error { message } => format!("(error: {})", message),
        }
    }
}

/// A commit's import provenance (git/p4 imports), tagged by `type`. Mirrors `fxv_cli_ux`'s
/// `ImportInfoDisplay`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum ImportInfoDisplay {
    Git {
        commit_hash: String,
        repo_id: Option<String>,
    },
    Perforce {
        changelist: u64,
        client_workspace: String,
        depot_path: String,
        stream: Option<String>,
    },
}

/// A single changed file, with the change kind on each axis that applies. Empty axes are omitted, so
/// a file with only an unpublished change serializes as just `{ path, unpublished_state }`.
/// `unpublished_state` is the change between the last published revision and the latest draft;
/// `workspace_state` is the not-yet-snapshotted change sitting in the working tree.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FileStatusJson {
    /// Workspace-relative path, always `/`-separated regardless of platform.
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unpublished_state: Option<ChangeKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_state: Option<ChangeKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict_state: Option<ConflictState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

/// The kind of change to a file. `MaybeChanged` only arises on the workspace axis (from a metadata
/// change whose content has not yet been hashed).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    MaybeChanged,
}

/// Why a file is in conflict. Present only on entries that are themselves conflicted, so a
/// directory that merely contains conflicts is not reported.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConflictState {
    pub kind: ConflictKind,
}

/// The cause of a conflict, as recovered from the entry the merge flagged and symmetrized against
/// the published parent tree.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
    /// Both sides changed the file content.
    Content,
    /// One side deleted the path and the other changed it.
    Deleted,
    /// One side has a file where the other has a directory.
    TypeChange,
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    pub(crate) fn sample_commit_ref(branch: &str, revision: u64) -> CommitRefJson {
        CommitRefJson {
            commit: CommitInfoJson {
                branch: branch.to_string(),
                revision: Some(revision),
                commit_type: CommitType::Published,
                draft_revision: None,
            },
            commit_hash: None,
            description: Some("Add feature X".to_string()),
            timestamp_millis_since_epoch_utc: 1700000000000,
            author_id: "user:1".to_string(),
            author_display_name: "Alice".to_string(),
            author_details: AuthorDisplayInfo::FxvUser {
                id: 1,
                username: "alice".to_string(),
                display_name: "Alice".to_string(),
            },
            import_info: None,
        }
    }

    pub(crate) fn sample_draft_commit_ref(branch: &str, revision: Option<u64>, draft_revision: u64) -> CommitRefJson {
        CommitRefJson {
            commit: CommitInfoJson {
                branch: branch.to_string(),
                revision,
                commit_type: CommitType::Draft,
                draft_revision: Some(draft_revision),
            },
            commit_hash: Some("00".repeat(32)),
            description: None,
            timestamp_millis_since_epoch_utc: 1699999000000,
            author_id: "user:1".to_string(),
            author_display_name: "Alice".to_string(),
            author_details: AuthorDisplayInfo::FxvUser {
                id: 1,
                username: "alice".to_string(),
                display_name: "Alice".to_string(),
            },
            import_info: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spec_string_formats() {
        let mut info = CommitInfoJson {
            branch: "main".to_string(),
            revision: Some(4),
            commit_type: CommitType::Published,
            draft_revision: None,
        };
        assert_eq!(info.to_spec_string(), "main.4");

        info.commit_type = CommitType::Draft;
        info.draft_revision = Some(2);
        assert_eq!(info.to_spec_string(), "main.4.2");

        info.revision = None;
        assert_eq!(info.to_spec_string(), "main.-.2");
    }

    #[test]
    fn test_commit_type_wire_names() {
        assert_eq!(serde_json::to_value(CommitType::Published).unwrap(), "published");
        assert_eq!(serde_json::to_value(CommitType::Draft).unwrap(), "draft");
    }

    #[test]
    fn test_change_kind_wire_names() {
        assert_eq!(serde_json::to_value(ChangeKind::MaybeChanged).unwrap(), "maybe_changed");
        let parsed: ChangeKind = serde_json::from_str("\"added\"").unwrap();
        assert_eq!(parsed, ChangeKind::Added);
    }

    #[test]
    fn test_commit_info_unparented_draft_omits_revision() {
        let json = serde_json::to_value(CommitInfoJson {
            branch: "main".to_string(),
            revision: None,
            commit_type: CommitType::Draft,
            draft_revision: Some(7),
        })
        .unwrap();
        assert!(json.get("revision").is_none());
        assert_eq!(json["type"], "draft");
        assert_eq!(json["draft_revision"], 7);
    }

    #[test]
    fn test_author_display_info_round_trip() {
        let variants = vec![
            AuthorDisplayInfo::Local,
            AuthorDisplayInfo::Service,
            AuthorDisplayInfo::FxvUser {
                id: 7,
                username: "carol".to_string(),
                display_name: "Carol".to_string(),
            },
            AuthorDisplayInfo::GitUser {
                name: "Dave".to_string(),
                email: "dave@example.com".to_string(),
            },
            AuthorDisplayInfo::P4User {
                username: "erin".to_string(),
                email: "erin@example.com".to_string(),
                display_name: "Erin".to_string(),
            },
            AuthorDisplayInfo::Error {
                message: "user id 99 not found".to_string(),
            },
        ];
        for variant in variants {
            let json = serde_json::to_value(&variant).unwrap();
            let parsed: AuthorDisplayInfo = serde_json::from_value(json.clone()).unwrap();
            assert_eq!(parsed, variant, "round-trip failed for {json}");
        }
    }

    #[test]
    fn test_file_status_omits_empty_axes() {
        let json = serde_json::to_value(FileStatusJson {
            path: "a/b.txt".to_string(),
            unpublished_state: Some(ChangeKind::Added),
            workspace_state: None,
            conflict_state: None,
            size: Some(12),
        })
        .unwrap();
        assert_eq!(json["unpublished_state"], "added");
        assert!(json.get("workspace_state").is_none());
        assert!(json.get("conflict_state").is_none());
    }
}
