//! Payload for `fxv user add/edit/deactivate` (`message.kind` of `user-add` / `user-edit` /
//! `user-deactivate`). Matches `schemas/user.schema.json` (`urn:fxv:schema:user:v1`).

// == External crates
use serde::{Deserialize, Serialize};

/// JSON payload for a completed user-management operation.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ManageUserJson {
    pub action: UserAction,
    /// Stable numeric identifier of the affected user.
    pub id: u32,
    pub username: String,
}

/// The management action that was performed. The CLI serializes this from a `&'static str`;
/// here it is a real enum so it can round-trip.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UserAction {
    Added,
    Edited,
    Deactivated,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::envelope::test_support::*;

    #[test]
    fn test_user_round_trip_and_schema() {
        for (action, kind) in [
            (UserAction::Added, "user-add"),
            (UserAction::Edited, "user-edit"),
            (UserAction::Deactivated, "user-deactivate"),
        ] {
            let payload = ManageUserJson {
                action,
                id: 7,
                username: "carol".to_string(),
            };
            let json = assert_payload_validates(kind, &payload);
            let parsed: ManageUserJson = serde_json::from_value(json["message"]["payload"].clone()).unwrap();
            assert_eq!(parsed, payload);
        }
    }
}
