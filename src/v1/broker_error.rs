//! Detail payload for a failed broker operation (`error_data.kind == "broker-error"`). Matches
//! `schemas/broker_error.schema.json` (`urn:fxv:schema:broker-error:v1`).
//!
//! Never a top-level message. It rides in an error envelope's `error_data`, so a consumer can tell a sign-in prompt
//! from a quota problem or an outdated `fxv` without parsing the message.

// == External crates
use serde::{Deserialize, Serialize};

/// Why a broker operation failed, and which broker, tenant and repository it was for when known.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BrokerErrorJson {
    pub reason: BrokerErrorReason,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broker: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// The HTTP status the broker answered with, absent when it was not reached.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
}

/// The kind of broker failure.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrokerErrorReason {
    NotAuthenticated,
    TokenExpired,
    Forbidden,
    QuotaExceeded,
    NetworkUnreachable,
    /// The broker no longer serves this `fxv` version. Comes with exit code 94.
    ClientUpgradeRequired,
    /// A reason added by a newer `fxv` than this crate knows. Never serialized by `fxv`.
    #[serde(other)]
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::envelope::{ErrorData, ErrorJson, ExitCode, MessageVersion, test_support::*};

    #[test]
    fn test_client_upgrade_required_rides_in_an_error_envelope() {
        let detail = BrokerErrorJson {
            reason: BrokerErrorReason::ClientUpgradeRequired,
            broker: Some("broker.example.invalid".to_string()),
            tenant: Some("acme".to_string()),
            repo: Some("game".to_string()),
            http_status: Some(400),
        };
        assert_value_validates("urn:fxv:schema:broker-error:v1", &detail);
        let payload = ErrorJson {
            message: "fxv 0.13.3 is older than 0.14.0, the oldest version this broker serves".to_string(),
            exit_code: ExitCode::ClientUpgradeRequired.into(),
            backtrace: None,
            error_data: Some(ErrorData {
                kind: "broker-error".to_string(),
                version: MessageVersion::new(1, 0),
                payload: serde_json::to_value(&detail).unwrap(),
            }),
        };

        let json = assert_payload_validates("error", &payload);
        assert_eq!(json["message"]["payload"]["exit_code"], 94);
        assert_eq!(
            json["message"]["payload"]["error_data"]["payload"]["reason"],
            "client_upgrade_required"
        );

        let parsed: ErrorJson = serde_json::from_value(json["message"]["payload"].clone()).unwrap();
        assert_eq!(parsed.exit_code(), ExitCode::ClientUpgradeRequired);
        let data = parsed.error_data.expect("error_data should survive the round trip");
        assert_eq!(data.kind, "broker-error");
        assert_eq!(serde_json::from_value::<BrokerErrorJson>(data.payload).unwrap(), detail);
    }

    #[test]
    fn test_unknown_reason_still_parses() {
        let detail: BrokerErrorJson =
            serde_json::from_value(serde_json::json!({"reason": "some_future_reason", "http_status": 503})).unwrap();
        assert_eq!(detail.reason, BrokerErrorReason::Other);
        assert_eq!(detail.http_status, Some(503));
    }

    #[test]
    fn test_minimal_detail_omits_unknown_fields() {
        let detail = BrokerErrorJson {
            reason: BrokerErrorReason::NetworkUnreachable,
            broker: None,
            tenant: None,
            repo: None,
            http_status: None,
        };
        assert_value_validates("urn:fxv:schema:broker-error:v1", &detail);
        assert_eq!(
            serde_json::to_value(&detail).unwrap(),
            serde_json::json!({ "reason": "network_unreachable" })
        );
    }
}
