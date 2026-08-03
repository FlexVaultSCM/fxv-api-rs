//! Payload for `fxv init` (`message.kind == "init"`). Matches `schemas/init.schema.json`
//! (`urn:fxv:schema:init:v1`).

// == External crates
use serde::{Deserialize, Serialize};

/// JSON payload for a completed `init` operation.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct InitJson {
    pub workspace_root: String,
    /// The repository the new workspace is connected to, or `null` for a `--local-only` workspace.
    pub repo_uri: Option<String>,
    pub local_only: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::envelope::test_support::*;

    #[test]
    fn test_init_round_trip_and_schema() {
        let payload = InitJson {
            workspace_root: "/work/mygame".to_string(),
            repo_uri: Some("s3://bucket/repo".to_string()),
            local_only: false,
        };
        let json = assert_payload_validates("init", &payload);
        let parsed: InitJson = serde_json::from_value(json["message"]["payload"].clone()).unwrap();
        assert_eq!(parsed, payload);
    }

    #[test]
    fn test_local_only_init_serializes_null_repo_uri() {
        // Unlike most optionals in this format, repo_uri has no skip attribute: it is always
        // present, null for local-only workspaces.
        let payload = InitJson {
            workspace_root: "/work/solo".to_string(),
            repo_uri: None,
            local_only: true,
        };
        let json = assert_payload_validates("init", &payload);
        assert!(json["message"]["payload"]["repo_uri"].is_null());
    }
}
