//! Shared local-filesystem conventions for the fxv family of clients (fxv-core, fxv-agent,
//! fxv-desktop, ...). Every client that reads or writes machine-local state (config, logs, the
//! cross-process workspace registry) must resolve to the same [`directories::ProjectDirs`]
//! identity for that state to actually land in the same place. Before this module existed, that
//! identity (`("", "", "fxv")`) was copy-pasted into each repo with comments asking whoever
//! edited one copy to keep the others in sync by hand - this is the one place it is spelled out
//! now, so a change here is a compile-time fact for every consumer rather than a social one.
//!
//! Deliberately its own top-level module, not nested under [`crate::v1`]: these are local
//! filesystem conventions, not part of the versioned JSON wire schema, so a schema version bump
//! has no bearing on this module and vice versa.

use std::path::PathBuf;

use directories::ProjectDirs;

/// The fxv family's shared `ProjectDirs` identity. Returns `None` if no home directory can be
/// resolved for the current user (rare - e.g. some minimal/containerized environments).
pub fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("", "", "fxv")
}

/// Where daily-rotating log files live, shared by every fxv client - each writes its own
/// `<prefix>.<date>.log` (a distinct `filename_prefix`, e.g. `fxv`, `fxv-agent`, `fxv-desktop`)
/// into this same directory, so logs from every client on a machine sit side by side:
/// - Linux: `$XDG_STATE_HOME/fxv/logs` or `~/.local/state/fxv/logs`
/// - Windows: `%LOCALAPPDATA%\fxv\data\logs`
/// - macOS: `~/Library/Application Support/fxv/logs`
pub fn logs_dir() -> Option<PathBuf> {
    let proj = project_dirs()?;
    Some(proj.state_dir().unwrap_or_else(|| proj.data_local_dir()).join("logs"))
}

/// Where a client's own roaming settings/config file should live:
/// - Linux: `$XDG_CONFIG_HOME/fxv` or `~/.config/fxv`
/// - Windows: `%APPDATA%\fxv\config`
/// - macOS: `~/Library/Application Support/fxv/config`
pub fn config_dir() -> Option<PathBuf> {
    Some(project_dirs()?.config_dir().to_path_buf())
}

/// Where machine-local (non-roaming) state should live - anything that only makes sense on the
/// machine that wrote it, such as the cross-process workspace registry `fxv` maintains and
/// `fxv-agent` reads:
/// - Linux: same as [`config_dir`] (XDG has no roaming/local split)
/// - Windows: `%LOCALAPPDATA%\fxv\config`
/// - macOS: same as [`config_dir`]
pub fn local_config_dir() -> Option<PathBuf> {
    Some(project_dirs()?.config_local_dir().to_path_buf())
}
