//! Payload for `fxv upgrade` (`message.kind == "upgrade"`). Matches `schemas/upgrade.schema.json`
//! (`urn:fxv:schema:upgrade:v1`).

// == External crates
use serde::{Deserialize, Serialize};

/// JSON payload for a completed `upgrade` command.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct UpgradeJson {
    /// Whether the binary was replaced. False means it was already the latest release.
    pub upgraded: bool,
    pub previous_version: String,
    /// The newest available release. Equal to `previous_version` when nothing was upgraded, and
    /// lower than it when the running binary is ahead of the latest published release.
    pub latest_version: String,
    /// Whether `latest_version` was flagged as a security or data-integrity fix. The human-facing
    /// critical banner only goes to stderr, so this is the only way a machine consumer learns it.
    pub critical: bool,
    /// Free-text detail about the release. Null rather than omitted when absent.
    pub message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::envelope::test_support::*;

    #[test]
    fn test_upgrade_round_trip_and_schema() {
        let payload = UpgradeJson {
            upgraded: true,
            previous_version: "0.8.4".to_string(),
            latest_version: "0.9.0".to_string(),
            critical: true,
            message: Some("Fixes a sync journal corruption.".to_string()),
        };
        let json = assert_payload_validates("upgrade", &payload);
        let parsed: UpgradeJson = serde_json::from_value(json["message"]["payload"].clone()).unwrap();
        assert_eq!(parsed, payload);
    }

    /// Nothing to do is still a success, and `message` is null rather than absent.
    #[test]
    fn test_already_up_to_date_keeps_a_null_message() {
        let payload = UpgradeJson {
            upgraded: false,
            previous_version: "0.9.0".to_string(),
            latest_version: "0.9.0".to_string(),
            critical: false,
            message: None,
        };
        let json = assert_payload_validates("upgrade", &payload);
        assert!(json["message"]["payload"]["message"].is_null());
    }
}
