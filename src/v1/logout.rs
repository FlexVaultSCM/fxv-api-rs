//! Payload for `fxv logout` (`message.kind == "logout"`). Matches `schemas/logout.schema.json`
//! (`urn:fxv:schema:logout:v1`).

// == External crates
use serde::{Deserialize, Serialize};

/// JSON payload for a completed `logout` command.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LogoutJson {
    /// Whether the workspace had a user logged in before this logout. Always present (unlike
    /// `username`) so this payload's shape is never mistaken for `login`'s identical-looking
    /// `{"username": "..."}` under the envelope schema's `oneOf` — a payload's shape must match
    /// exactly one command's schema.
    pub was_logged_in: bool,
    /// The username that was logged out, if the workspace was logged in. Omitted (rather than
    /// `null`) when the workspace was already logged out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::envelope::test_support::*;

    #[test]
    fn test_logout_round_trip_and_schema() {
        let payload = LogoutJson {
            was_logged_in: true,
            username: Some("alice".to_string()),
        };
        let json = assert_payload_validates("logout", &payload);
        let parsed: LogoutJson = serde_json::from_value(json["message"]["payload"].clone()).unwrap();
        assert_eq!(parsed, payload);
    }

    #[test]
    fn test_already_logged_out_omits_username() {
        let payload = LogoutJson {
            was_logged_in: false,
            username: None,
        };
        let json = assert_payload_validates("logout", &payload);
        assert!(json["message"]["payload"].get("username").is_none());
    }
}
