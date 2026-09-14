# AGENTS.md

Bootstrap context for agents working in `fxv-api-rs`. This is a living document — add non-obvious, expensive-to-discover context judiciously.

## What this crate is

`fxv-api` (repo name `fxv-api-rs`) is the shared Rust model of the FlexVault CLI's JSON wire format. The `fxv` CLI (built from the sibling repo `../fxv-core`) emits JSON under `--format json`; this crate holds deserializable mirrors of those payload structs plus the JSON Schemas that define the contract.

History: v0.1 of this crate was a hand-designed mock workspace API (`WorkspaceApi` trait, `MockWorkspaceApi`, a lazy `Directory` tree). That surface was deleted in v0.2 and replaced with the CLI wire types. The only consumer is `../fxv-desktop`.

## The duplication contract (important)

The structs in `src/v1/` are **duplicated by hand** from `fxv-core`:

- `fxv-core/crates/fxv_cli/src/view/{json_envelope,commit_info_json,status,history_json,change_info_json,doctor,init,login,logout,manage_user,workspace_sync}.rs`
- `fxv-core/crates/fxv_cli_ux/src/{author_resolver,import_info}.rs` (`AuthorDisplayInfo`, `ImportInfoDisplay`)

`schemas/*.schema.json` is likewise a copy of `fxv-core/crates/fxv_cli/schemas/`. **Any change to the CLI's JSON output must be mirrored here, and vice versa**, until fxv-core is converted to depend on this crate (planned, not started). Both repos validate against their own schema copy in tests, so drift shows up as a test failure on whichever side changed — but only if the schemas are re-copied. When syncing, copy the schema files verbatim.

Deliberate differences from the CLI-side structs (which are serialize-only):

- Everything here also derives `Deserialize`, `Debug`, `Clone`, `PartialEq`.
- CLI `&'static str` fields are real enums here: `common::CommitType`, `user::UserAction`, `change_info::FileChangeAction`, `doctor::DoctorCheckStatus`. `ProgramMetadata.name/version` are `String`, and `MessageEnvelope.version` is a `MessageVersion` that parses the CLI's `"major.minor"` string.
- `Vec` fields the CLI omits when empty carry `#[serde(default)]` so they parse back.

## Layout

- `src/common.rs` — `RelativePath`, a normalized `/`-separated workspace-relative path newtype with component-wise ordering (deliberate: byte order mis-sorts; see its tests). Survives from v0.1.
- `src/v1/envelope.rs` — `OutputEnvelope<T>`/`MessageEnvelope<T>`/`ProgramMetadata`, `ErrorJson`, `ExitCode` (0/1/98/99 = ok/general/interrupted-sync/workspace-locked), and `parse_output<T>()` which discriminates success from `kind == "error"`. Also `#[cfg(test)] test_support` with the schema validator used by every module's tests.
- `src/v1/common.rs` — shared payload types (`CommitRefJson`, `CommitInfoJson`, `AuthorDisplayInfo`, `ImportInfoDisplay`, `FileStatusJson`, `ChangeKind`, `ConflictState`/`ConflictKind`) matching `common.schema.json`'s `$defs`.
- `src/v1/{status,history,change_info,doctor,init,login,logout,upgrade,user,workspace_sync}.rs` — one module per command payload. `workspace_sync` is shared by the kinds `sync`/`goto`/`resolve`/`revert`.
- `src/v1/interrupted_sync.rs` — the one payload that is never a top-level message. It rides in an error envelope's `error_data` at exit code 98, under its own kind and version.
- `tests/fixtures/*.json` + `tests/fixture_test.rs` — envelopes captured from a real `fxv` binary, parsed with the typed structs.

## Wire-format facts that bite

- There is no success flag; success vs. error is `message.kind == "error"` plus the process exit code. Error envelopes go to **stdout**, like successes.
- An error payload may include `error_data`, a nested kind/version/payload triple versioned on its own timeline. Only `interrupted-sync` exists today, and only at exit code 98. `payload` stays a `serde_json::Value` here: the caller matches on `kind` before deserializing it.
- `message.version` is the payload schema's `major.minor` version **on that `kind`'s own timeline**, not a global one — `status` is at `2.0` and every other kind is at `1.0` today. It is a string so `1.10` can't be read as `1.1`; `MessageVersion` parses it. On the CLI side each payload's `VersionedJsonMessage::version()` supplies it, and fxv-core's `cargo insta` snapshots fail if a payload's shape moves without the version moving.
- The envelope schema's payload `oneOf` requires payload shapes to be mutually exclusive across commands — that is why `LogoutJson` always carries `was_logged_in` (else it would collide with `LoginJson`). Keep new payloads unambiguous.
- Schemas are sealed against extra fields (`additionalProperties: false`, or `unevaluatedProperties: false` where an `allOf` composes `commitRefFields`); the drift-guard test in `history.rs` asserts an extra field fails validation. Don't weaken either.
- A commit reference comes in two schema flavours: `commitRef` is the sealed whole-value form (history entries, status's head commits, sync's created revisions); `commitRefFields` is the deliberately unsealed form that `changeinfo` composes under `allOf` because it flattens the reference alongside its own fields. Both map to the one `CommitRefJson`.
- `history` entries never include `commit_hash` even though `CommitRefJson` has the field — the schema rejects it there.
- `CommitInfoJson.revision: None` means an *unparented* draft (spec `main.-.N`), distinct from revision 0.
- Paths in payloads are `/`-separated, normalized by the CLI on every platform, with one exception noted below (`interrupted-sync`).
- `-progress` kinds and `update_frequency_seconds`/`sequence` are reserved for future JSONL streaming; the CLI never emits them today (`json-l` is `unimplemented!()`).
- Known upstream drift: `common.schema.json`'s `importInfo` Perforce variant requires `user_name`, which the Rust struct doesn't have. Untested upstream because no fixture produces a Perforce import. Fix belongs in fxv-core.
- Known upstream drift: `interrupted-sync`'s `sampled_unfinished_paths` keeps the platform separator (`big\asset045.bin` on Windows) where every other payload normalizes to `/`. The CLI builds them with `to_string_lossy()` on raw `PathBuf`s. `tests/fixtures/interrupted_sync_error.json` records the behavior as captured; the fix belongs in fxv-core.

## JSON-capable commands (as of fxv-core 2026-08)

`status`, `history`, `changeinfo`, `init`, `login`, `logout`, `upgrade`, `user add/edit/deactivate`, `sync`, `goto`, `resolve`, `revert`, `doctor` (and `doctor bundle`, same `doctor` kind), plus the `error` envelope. **Not** JSON-capable: `publish`, `snapshot`, `diff`, `clone`, `branch`, `repo *`, `util *`.

## Build & style

Plain stable Rust, edition 2024, no workspace. `cargo test` runs everything (needs `schemas/` on disk — tests load via `CARGO_MANIFEST_DIR`). `cargo fmt` before committing; `rustfmt.toml` matches fxv-core's 120-col, crate-granularity imports (granularity silently no-ops on stable). Follow fxv-core's `README.md` code style: import group headers (`// == Std`, `// == Internal crates`, `// == External crates`), newtypes over bare strings, terse inline comments.

## Working with the user

Same rules as `../fxv-core/AGENTS.md`: small steps with explicit signoff between them, never commit without consent, facts over sycophancy, prefer inlining over single-use helpers.
