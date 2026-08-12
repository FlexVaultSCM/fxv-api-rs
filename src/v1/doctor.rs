//! Payload for `fxv doctor` and `fxv doctor bundle` (both `message.kind == "doctor"`). Matches
//! `schemas/doctor.schema.json` (`urn:fxv:schema:doctor:v1`).

// == External crates
use serde::{Deserialize, Serialize};

/// JSON payload for `fxv doctor`: the diagnostic checks that ran, and their outcome counts.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DoctorJson {
    pub checks: Vec<DoctorCheckJson>,
    pub summary: DoctorSummaryJson,
    /// Only present for `fxv doctor bundle` — `fxv doctor` on its own omits it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle: Option<DoctorBundleJson>,
}

/// The support archive `fxv doctor bundle` wrote.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DoctorBundleJson {
    pub path: String,
    pub size_bytes: u64,
}

/// One diagnostic check that was run or skipped.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DoctorCheckJson {
    /// Stable machine identifier, e.g. `workspace.lock`.
    pub id: String,
    /// Which check group this belongs to, e.g. `environment`, `agent`, `workspace`, `repo`.
    pub group: String,
    pub name: String,
    pub status: DoctorCheckStatus,
    pub detail: String,
    /// Suggested remediation, present when the check is actionable (`warn`/`fail`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_hint: Option<String>,
    /// Present only when `--fix` ran: whether a remediation was actually applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixed: Option<bool>,
}

/// Outcome of a single check. The CLI serializes this from a `&'static str`; here it is a real enum
/// so it can round-trip.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DoctorCheckStatus {
    Ok,
    Warn,
    Fail,
    Skipped,
}

/// Counts of checks by outcome across the whole run.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct DoctorSummaryJson {
    pub ok: usize,
    pub warn: usize,
    pub fail: usize,
    pub skipped: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::envelope::test_support::*;

    #[test]
    fn test_doctor_round_trip_and_schema() {
        let payload = DoctorJson {
            checks: vec![
                DoctorCheckJson {
                    id: "environment.disk_space".to_string(),
                    group: "environment".to_string(),
                    name: "Free disk space".to_string(),
                    status: DoctorCheckStatus::Ok,
                    detail: "412 GiB free".to_string(),
                    fix_hint: None,
                    fixed: None,
                },
                DoctorCheckJson {
                    id: "workspace.lock".to_string(),
                    group: "workspace".to_string(),
                    name: "Workspace lock".to_string(),
                    status: DoctorCheckStatus::Warn,
                    detail: "A stale lock file is present".to_string(),
                    fix_hint: Some("Run `fxv doctor --fix`".to_string()),
                    fixed: Some(true),
                },
            ],
            summary: DoctorSummaryJson {
                ok: 1,
                warn: 1,
                fail: 0,
                skipped: 0,
            },
            bundle: Some(DoctorBundleJson {
                path: "/tmp/fxv-doctor-bundle.zip".to_string(),
                size_bytes: 20480,
            }),
        };
        let json = assert_payload_validates("doctor", &payload);
        let parsed: DoctorJson = serde_json::from_value(json["message"]["payload"].clone()).unwrap();
        assert_eq!(parsed, payload);
        assert_eq!(json["message"]["payload"]["checks"][1]["status"], "warn");
    }

    #[test]
    fn test_bundle_omitted_for_plain_doctor() {
        let payload = DoctorJson {
            checks: vec![],
            summary: DoctorSummaryJson::default(),
            bundle: None,
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert!(json.get("bundle").is_none());
        assert_eq!(serde_json::from_value::<DoctorJson>(json).unwrap(), payload);
    }
}
