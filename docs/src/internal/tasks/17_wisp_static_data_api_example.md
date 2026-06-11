# Wisp developer reference API example tasks

## Goal

Compile and run a Wisp-style Wasm API for Cloudflare Workers that serves a
small developer reference catalog.

## Milestone 1: acceptance shape

- [ ] Add `examples/wisp_static_api/` with `gleam.toml`, source layout, and a
      minimal Worker host adapter.
- [ ] Include initial datasets for gitignore, gitattributes, licenses, SPDX,
      MIME types, HTTP status codes, languages, and cron presets.
- [ ] Define routes, exported functions, and host imports in a short ABI note.
- [ ] Add a compile-only fixture that captures the first unsupported compiler
      diagnostic.

## Milestone 2: project and dependency support

- [ ] Reuse project compilation from the Lustre example milestone.
- [ ] Load enough dependency metadata for Wisp and direct support packages.
- [ ] Support dependency module interfaces used by routing, request parsing,
      and response construction.
- [ ] Report unsupported dependency and ABI shapes with source spans.
- [ ] Add tests for cross-module route helpers and dependency imports.

## Milestone 3: Worker ABI

- [ ] Add a `workers` or `browser` target decision for Cloudflare Worker Wasm.
- [ ] Lower general external functions to Worker-compatible Wasm imports.
- [ ] Start with string request inputs and string or tagged response outputs.
- [ ] Add adapter helpers for status, headers, and body data when needed.
- [ ] Add a Gleam route table fixture that returns tagged response data without
      Worker-specific request or response handles.
- [ ] Validate that unsupported request or response handles fail before emit.

## Milestone 4: static data

- [ ] Decide whether static data is handwritten Gleam or generated Gleam.
- [ ] Keep generated sources deterministic and reviewable.
- [ ] Implement list and detail routes for gitignore templates.
- [ ] Implement list and detail routes for gitattributes templates.
- [ ] Implement list and detail routes for licenses and SPDX metadata.
- [ ] Implement lookup routes for MIME types and HTTP status codes.
- [ ] Implement list and detail routes for languages and cron presets.
- [ ] Add not-found and bad-request responses.
- [ ] Add tests for route matching and returned data.

## Milestone 5: deployable Worker host

- [ ] Add a minimal JS Worker that loads the Wasm module and forwards requests.
- [ ] Document local `wrangler` development and deployment commands.
- [ ] Add a smoke test for Worker-style request handling where practical.
- [ ] Ensure the emitted Wasm artifact path is stable for the host adapter.

## Milestone 6: regression coverage

- [ ] Add a compile fixture for the full static API example.
- [ ] Snapshot import/export metadata for the Worker-target Wasm.
- [ ] Add diagnostics tests for unsupported Worker ABI shapes.
- [ ] Keep unsupported follow-up work recorded in the active task file.

## Done when

The example builds from its project directory, can be called by a Cloudflare
Worker host, and returns static data without custom manual memory handling in
the host.
