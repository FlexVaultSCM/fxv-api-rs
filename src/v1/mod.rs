//! Version 1 of the FlexVault CLI JSON wire format.
//!
//! These types mirror, field for field, the serde structs the `fxv` CLI uses to produce
//! `--format json` output (defined in `fxv-core`'s `fxv_cli`/`fxv_cli_ux` crates). The JSON Schemas
//! in this repo's `schemas/` directory are copied from `fxv-core/crates/fxv_cli/schemas/` and are
//! the contract both sides are tested against. Until `fxv-core` depends on this crate directly,
//! any change to the CLI's JSON output must be mirrored here (and vice versa).
//!
//! Deliberate differences from the CLI-side structs, which are serialize-only:
//! - Every type here also derives `Deserialize`, `Debug`, `Clone`, and `PartialEq`.
//! - `&'static str` fields became real enums: [`common::CommitType`], [`user::UserAction`],
//!   [`change_info::FileChangeAction`], [`doctor::DoctorCheckStatus`]; `ProgramMetadata`'s `name`/`version` became
//!   `String`.
//! - `message.version` is an [`envelope::MessageVersion`] that parses the CLI's `"major.minor"` string, rather than the
//!   CLI's write-only equivalent.

pub mod change_info;
pub mod common;
pub mod doctor;
pub mod envelope;
pub mod history;
pub mod init;
pub mod interrupted_sync;
pub mod login;
pub mod logout;
pub mod status;
pub mod upgrade;
pub mod user;
pub mod workspace_sync;
