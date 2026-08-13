# FlexVault API (Rust)

FlexVault is a turbocharged, user-friendly source control management system. This crate (`fxv-api`) is the Rust model of the FlexVault CLI's JSON wire format, for building integrations that drive the `fxv` binary programmatically.

The `fxv` CLI emits a JSON envelope on stdout for supported commands when invoked with `--format json`. This crate provides:

- **Typed payloads** (`v1` module): serde `Serialize`/`Deserialize` structs mirroring every JSON-capable command's output — `status`, `history`, `changeinfo`, `init`, `login`, `logout`, `user add/edit/deactivate`, `doctor`, and the `sync`/`goto`/`resolve`/`revert` family — plus the envelope itself and the `error` payload.
- **`v1::envelope::parse_output<T>()`**: parses one stdout envelope, discriminating success from a CLI-reported error (`message.kind == "error"`).
- **JSON Schemas** (`schemas/`): the authoritative contract, copied from `fxv-core/crates/fxv_cli/schemas/`. Tests validate every type against them, and `tests/fixtures/` holds envelopes captured from a real `fxv` binary that must round-trip value-identically.
- **`common::RelativePath`**: a normalized, `/`-separated, workspace-relative path newtype with component-wise ordering.

## Current state

- Version 1 of the wire format lives in the `v1` module and is unstable (the CLI is pre-1.0).
- The structs are currently duplicated by hand from `fxv-core`; changes on either side must be mirrored. See `AGENTS.md` for the duplication contract and wire-format details.

## Testing and development

```bash
cargo test    # unit round-trip/schema tests + captured-fixture integration tests
cargo fmt     # 120-col rustfmt config, required before committing
```

To regenerate `tests/fixtures/`, build `fxv` in the sibling `fxv-core` checkout and re-run the capture steps described in `tests/fixture_test.rs`.
