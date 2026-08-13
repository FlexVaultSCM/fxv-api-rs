//! Payload for `fxv login` (`message.kind == "login"`). Matches `schemas/login.schema.json`
//! (`urn:fxv:schema:login:v1`).

// == External crates
use serde::{Deserialize, Serialize};

/// JSON payload for a completed `login` operation.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LoginJson {
    pub username: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::envelope::test_support::*;

    #[test]
    fn test_login_round_trip_and_schema() {
        let payload = LoginJson {
            username: "alice".to_string(),
        };
        let json = assert_payload_validates("login", &payload);
        let parsed: LoginJson = serde_json::from_value(json["message"]["payload"].clone()).unwrap();
        assert_eq!(parsed, payload);
    }
}
