# Internal Docs

This directory holds task trackers and specs for in-flight Regulus work.

The public/user docs live in `docs/website`.

The teaching book lives in `docs/book`.

## Status

Regulus can compile supported Gleam projects from `gleam.toml` into one linked
Wasm artifact.

Current work is focused on making that Gleam-project-to-Wasm path more usable:

1. Finish the JavaScript host ABI pieces needed by Wasm projects.
2. Harden runtime memory and host pointer behavior.
3. Improve CLI artifacts, diagnostics, and host metadata.
4. Fill stdlib and dependency gaps that block realistic projects.
5. Add larger examples as acceptance fixtures after the core gaps are planned.

## Specs

- [JS host ABI](specs/15_js_host_abi.md)
- [Stdlib and host interop](specs/16_stdlib_and_host_interop.md)
- [CLI and build outputs](specs/17_cli_and_build_outputs.md)
- [Example projects](specs/18_example_projects.md)
- [Runtime memory hardening](specs/19_runtime_memory_hardening.md)
- [WASI host ABI](specs/20_wasi_host_abi.md)

## Task Trackers

- [JS host ABI](tasks/15_js_host_abi.md)
- [Stdlib and host interop](tasks/16_stdlib_and_host_interop.md)
- [CLI and build outputs](tasks/17_cli_and_build_outputs.md)
- [Runtime memory hardening](tasks/20_runtime_memory_hardening.md)
- [WASI host ABI](tasks/21_wasi_host_abi.md)

### Examples

- [Lustre weather SPA](tasks/18_lustre_weather_example.md)
- [Wisp Dev API](tasks/19_wisp_static_data_api_example.md)

[compiling-projects]: ../website/reference/compiling-projects.md
[projects-design]: ../website/development/projects.md
