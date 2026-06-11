# Lustre weather example tasks

## Goal

Compile and run a local-first Lustre browser app that shows simple NWS
forecasts for selected US cities.

## Milestone 1: acceptance shape

- [ ] Add `examples/lustre_weather/` with `gleam.toml`, source layout, and a
      minimal browser host.
- [ ] Choose the first static city table and forecast fields.
- [ ] Define the example's imports and exports in a short host ABI note.
- [ ] Add a compile-only fixture that captures the first unsupported compiler
      diagnostic.

## Milestone 2: project compilation

- [ ] Add a CLI command or flag that compiles a `gleam.toml` project to Wasm.
- [ ] Parse, target-select, resolve, type-check, lower, and emit all project
      modules in deterministic order.
- [ ] Link same-project module imports without requiring host imports.
- [ ] Emit stable output names for the example module and final artifact.
- [ ] Add project compile integration tests for success and diagnostics.

## Milestone 3: dependency interfaces

- [ ] Load enough dependency metadata for Lustre and its direct support
      packages.
- [ ] Support dependency module interfaces for values, types, constructors, and
      labels used by the example.
- [ ] Report unsupported dependency members before lowering.
- [ ] Add focused tests for dependency interface lookup and visibility.

## Milestone 4: browser host imports

- [ ] Lower general external functions to target-specific Wasm imports.
- [ ] Validate browser external modules and names before byte emission.
- [ ] Add host adapters for string input and output across browser imports.
- [ ] Add example imports for fetch text, local storage read/write, and online
      state if the app needs them.
- [ ] Add browser-target tests that inspect import names and ABI diagnostics.

## Milestone 5: app feature slice

- [ ] Render city selection and cached forecast state through Lustre.
- [ ] Fetch the NWS forecast endpoint through the browser host.
- [ ] Add a small forecast decoder module that uses `gleam/dynamic/decode`.
- [ ] Add a compile fixture for the forecast decoder and snapshot the first
      unsupported language, dependency, dynamic runtime, or ABI diagnostic.
- [ ] Persist the selected city and last successful forecast locally.
- [ ] Add an example README with build and run commands.

## Milestone 6: regression coverage

- [ ] Add a compile fixture for the full weather example.
- [ ] Add a host smoke test that instantiates the browser-target Wasm module.
- [ ] Snapshot WAT or import/export metadata for the app.
- [ ] Keep unsupported follow-up work recorded in the active task file.

## Done when

The example builds from its project directory, runs in a browser host, and
documents every host import and exported Wasm function it relies on.
