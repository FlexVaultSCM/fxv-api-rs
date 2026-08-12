//! Payload for `fxv history` (`message.kind == "history"`). Matches `schemas/history.schema.json`
//! (`urn:fxv:schema:history:v1`).

// == Internal crates
use crate::v1::common::CommitRefJson;

// == External crates
use serde::{Deserialize, Serialize};

/// JSON payload for `fxv history`: newest-first revision entries. An entry carries nothing beyond
/// the commit reference itself, so entries are plain [`CommitRefJson`]s.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct HistoryOutputJson {
    pub entries: Vec<CommitRefJson>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::{common::test_support::*, envelope::test_support::*};

    #[test]
    fn test_history_round_trip_and_schema() {
        let payload = HistoryOutputJson {
            entries: vec![
                sample_commit_ref("main", 3),
                // History entries never include commit_hash; the schema rejects it.
                CommitRefJson {
                    commit_hash: None,
                    ..sample_draft_commit_ref("main", Some(2), 2)
                },
            ],
        };
        let json = assert_payload_validates("history", &payload);
        let parsed: HistoryOutputJson = serde_json::from_value(json["message"]["payload"].clone()).unwrap();
        assert_eq!(parsed, payload);
    }

    /// Guards the drift-detection mechanism itself: a payload with an extra field must fail
    /// validation. If this test ever passes, the schema's sealing (`additionalProperties` /
    /// `unevaluatedProperties`) was weakened and drift is no longer caught.
    #[test]
    fn test_extra_field_fails_validation() {
        let mut json = serde_json::to_value(HistoryOutputJson {
            entries: vec![sample_commit_ref("main", 1)],
        })
        .unwrap();
        json["entries"][0]["unexpected_field"] = "drift".into();

        let envelope = serde_json::json!({
            "program": serde_json::to_value(sample_program_metadata()).unwrap(),
            "message": { "kind": "history", "version": "1.0", "payload": json },
        });
        let validator = build_envelope_validator();
        let errors: Vec<_> = validator.iter_errors(&envelope).collect();
        assert!(
            !errors.is_empty(),
            "Extra field was accepted; schema drift detection is broken"
        );
    }
}
