# FlexVault API (Rust)

FlexVault is a turbocharged, user-friendly source control management system. This crate exposes the Rust API surface for interacting with FlexVault in a programmatic manner, specifically for building integrations.

## Current State
- The crate is structured to be versioned across expected iterations of the API.  Currently only version 1 exists, in the `v1` module, and this is unstable.
- The async `WorkspaceApi` trait in `src/<VERSION>/client.rs` defines the API surface.
- An optional mock client and associated data generators exist to create data for testing.

## Core Cargo Features
- `serde` (default) derives serialization for key workspace types so you can move data across the wire or cache it.

You can disable defaults with `--no-default-features` and then re enable specific pieces, for example `cargo build --no-default-features --features serde`.

## Testing and development
- Enabling the `mock_client` feature builds `v1::mock_client::MockWorkspaceApi`, which can be used to simulate FlexVault based on static local data. It is customizable to simulate a delay on each API call to validate slow loading scenarios.
- Enabling `mock_data_generator` feature builds the `mock_data_generator` tool, enabling filesystem snapshots for use with the mock client. Generated data assumes unchanged, conflict free files unless you edit it by hand.

### Using `mock_data_generator`

Use the `mock_data_generator` binary to capture sample trees:

```
mock_data_generator path/to/capture > tree.json
```

- Add `--compact` or `-c` for single line JSON output.
- Use the generated json to populate your `MockWorkspaceApi` for testing.
