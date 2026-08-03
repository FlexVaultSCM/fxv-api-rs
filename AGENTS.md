# AGENTS.md

Bootstrap context for agents working in `fxv-api-rs`. This is a living document — add non-obvious, expensive-to-discover context judiciously.

## What this crate is

`fxv-api` (repo name `fxv-api-rs`) is the shared Rust model of the FlexVault CLI's JSON wire format. The `fxv` CLI (built from the sibling repo `../fxv-core`) emits JSON under `--format json`; this crate holds deserializable mirrors of those payload structs plus the JSON Schemas that define the contract.

History: v0.1 of this crate was a hand-designed mock workspace API (`WorkspaceApi` trait, `MockWorkspaceApi`, a lazy `Directory` tree). That surface was deleted in v0.2 and replaced with the CLI wire types. The only consumer is `../fxv-desktop`.

## The duplication contract (important)

The structs in `src/v1/` are **duplicated by hand** from `fxv-core`:

- `fxv-core/crates/fxv_cli/src/view/{json_envelope,commit_info_json,status,history_json,change_info_json,init,login,logout,manage_user,workspace_sync}.rs`
- `fxv-core/crates/fxv_cli_ux/src/{author_resolver,import_info}.rs` (`AuthorDisplayInfo`, `ImportInfoDisplay`)

`schemas/*.schema.json` is likewise a copy of `fxv-core/crates/fxv_cli/schemas/`. **Any change to the CLI's JSON output must be mirrored here, and vice versa**, until fxv-core is converted to depend on this crate (planned, not started). Both repos validate against their own schema copy in tests, so drift shows up as a test failure on whichever side changed — but only if the schemas are re-copied. When syncing, copy the schema files verbatim.

Deliberate differences from the CLI-side structs (which are serialize-only):

- Everything here also derives `Deserialize`, `Debug`, `Clone`, `PartialEq`.
- CLI `&'static str` fields are real enums here: `common::CommitType`, `user::UserAction`, `change_info::FileChangeAction`. `ProgramMetadata.name/version` are `String`.
- `Vec` fields the CLI omits when empty carry `#[serde(default)]` so they parse back.

## Layout

- `src/common.rs` — `RelativePath`, a normalized `/`-separated workspace-relative path newtype with component-wise ordering (deliberate: byte order mis-sorts; see its tests). Survives from v0.1.
- `src/v1/envelope.rs` — `OutputEnvelope<T>`/`MessageEnvelope<T>`/`ProgramMetadata`, `ErrorJson`, `ExitCode` (0/1/99 = ok/general/workspace-locked), and `parse_output<T>()` which discriminates success from `kind == "error"`. Also `#[cfg(test)] test_support` with the schema validator used by every module's tests.
- `src/v1/common.rs` — shared payload types (`CommitRefJson`, `CommitInfoJson`, `AuthorDisplayInfo`, `ImportInfoDisplay`, `FileStatusJson`, `ChangeKind`) matching `common.schema.json`'s `$defs`.
- `src/v1/{status,history,change_info,init,login,logout,user,workspace_sync}.rs` — one module per command payload. `workspace_sync` is shared by the kinds `sync`/`goto`/`resolve`/`revert`.
- `tests/fixtures/*.json` + `tests/fixture_test.rs` — envelopes captured from a real `fxv` binary, parsed with the typed structs.

## Wire-format facts that bite

- There is no success flag and no schema-version field; success vs. error is `message.kind == "error"` plus the process exit code. Error envelopes go to **stdout**, like successes.
- The envelope schema's payload `oneOf` requires payload shapes to be mutually exclusive across commands — that is why `LogoutJson` always carries `was_logged_in` (else it would collide with `LoginJson`). Keep new payloads unambiguous.
- All schemas use `additionalProperties: false`; the drift-guard test in `history.rs` asserts an extra field fails validation. Don't weaken either.
- `history` entries never include `commit_hash` even though `CommitRefJson` has the field — the schema rejects it there.
- `CommitInfoJson.revision: None` means an *unparented* draft (spec `main.-.N`), distinct from revision 0.
- Paths in payloads are always `/`-separated, normalized by the CLI on every platform.
- `-progress` kinds and `update_frequency_seconds`/`sequence` are reserved for future JSONL streaming; the CLI never emits them today (`json-l` is `unimplemented!()`).
- Known upstream drift: `common.schema.json`'s `importInfo` Perforce variant requires `user_name`, which the Rust struct doesn't have. Untested upstream because no fixture produces a Perforce import. Fix belongs in fxv-core.

## JSON-capable commands (as of fxv-core 2026-08)

`status`, `history`, `changeinfo`, `init`, `login`, `logout`, `user add/edit/deactivate`, `sync`, `goto`, `resolve`, `revert`, plus the `error` envelope. **Not** JSON-capable: `publish`, `snapshot`, `diff`, `clone`, `branch`, `repo *`, `util *`.

## Build & style

Plain stable Rust, edition 2024, no workspace. `cargo test` runs everything (needs `schemas/` on disk — tests load via `CARGO_MANIFEST_DIR`). `cargo fmt` before committing; `rustfmt.toml` matches fxv-core's 120-col, crate-granularity imports (granularity silently no-ops on stable). Follow fxv-core's `README.md` code style: import group headers (`// == Std`, `// == Internal crates`, `// == External crates`), newtypes over bare strings, terse inline comments.

## Working with the user

Same rules as `../fxv-core/AGENTS.md`: small steps with explicit signoff between them, never commit without consent, facts over sycophancy, prefer inlining over single-use helpers.
