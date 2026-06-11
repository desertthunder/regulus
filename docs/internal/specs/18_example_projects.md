# Example projects

Regulus needs real examples that prove the compiler can handle useful Gleam
projects, browser-oriented Wasm, and JS-hosted server Wasm. The first examples
should be small enough to maintain, but large enough to expose missing compiler
capabilities before they become design debt.

## Goals

1. Build a local-first Lustre single-page app that shows simple National
   Weather Service forecasts for US cities.
2. Build a Wisp-style Wasm API deployable to a JS-hosted server environment
   that serves a small developer reference API.
3. Use the examples as compiler acceptance fixtures, not as separate product
   projects.
4. Keep host boundaries explicit and target-specific.
5. Prefer general language, dependency, runtime, and ABI support over
   example-specific compiler behavior.

## Current compiler gaps

The current CLI compiles one Gleam source file. `project` loads `gleam.toml` and
prints discovered modules, but it does not compile or link a whole project.

The project model records dependency requirements, but the compiler does not
load Hex packages, path dependencies, or dependency module interfaces. Lustre,
Wisp, JSON helpers, HTTP helpers, and any example-local support modules require
that work.

External functions are parsed, resolved, and type-checked, but lowering only
materializes current stdlib host calls. General external functions need import
lowering, target validation, and ABI diagnostics.

The host ABI supports scalar values and managed pointers. JavaScript examples
need stable adapter helpers for reading and writing strings, lists, records,
result values, and host objects such as requests or responses.

`gleam/dynamic`, `gleam/dynamic/decode`, `gleam/uri`, `gleam/pair`,
`gleam/set`, `gleam/string_tree`, and `gleam/bytes_tree` are still unsupported
or interface-only. The examples should avoid unnecessary breadth, but JSON and
HTTP work should be handled by compiling library code where possible plus a
small dynamic-value or host bridge where necessary.

## Lustre weather SPA

The weather app should be a local-first browser example. It should compile a
Lustre app to Wasm, load it from a small JS host, and use the NWS Weather API
for forecasts based on selected US cities.

The first version should avoid geocoding. It can ship a static city table with
city names and known NWS grid points. The app stores the selected city and the
last successful forecast locally, then refreshes from the network when the
browser host is online.

The compiler should not need to implement browser networking directly in
Gleam. A small target-specific host import can provide `fetch_text`, local
storage reads and writes, and time or online state as needed. The Gleam side
should keep those imports behind an example module with explicit types. The
compiler should only provide general external-function lowering, target
validation, ABI checks, and runtime adapters.

The example is useful when it proves:

1. Whole-project browser compilation works.
2. Lustre dependency interfaces can be loaded or modeled.
3. Target-specific browser externals lower to Wasm imports.
4. String and simple structured values cross the host boundary predictably.
5. Unsupported dependency or ABI shapes fail with source-spanned diagnostics.

## Wisp developer reference API

The Wisp example should compile to a Wasm module that a JS-hosted server can
call for route handling or route data. The first version should serve a small
static developer reference catalog.

The initial route surface should stay intentionally small:

1. `GET /gitignore`
2. `GET /gitignore/:name`
3. `GET /gitattributes`
4. `GET /gitattributes/:name`
5. `GET /licenses`
6. `GET /licenses/:id`
7. `GET /spdx`
8. `GET /spdx/:id`
9. `GET /mime/:extension`
10. `GET /http/status/:code`
11. `GET /languages`
12. `GET /languages/:name`
13. `GET /cron`
14. `GET /cron/:name`

The datasets should cover gitignore templates, gitattributes templates, common
licenses, SPDX metadata, MIME type lookup, HTTP status metadata, language
metadata, and common cron presets.

Static content can be embedded in Gleam source or generated into Gleam modules
at build time. Generated sources must be deterministic and small enough for
review.

The JS host can own the real request and response objects. The Wasm API should
start with simple string inputs and structured return data, then graduate to
opaque request and response handles only when needed. Routing, static data
lookup, and response shaping should live in compiled Gleam code, not in
compiler-owned special cases.

The example is useful when it proves:

1. Whole-project JS-hosted server compilation works.
2. Wisp dependency interfaces can be loaded or modeled.
3. Bundler-profile externals lower to Wasm imports.
4. Static data can be embedded without brittle manual memory handling.
5. The CLI can emit deployable Wasm plus minimal JS host glue.

## Acceptance

Both examples must be built from normal project directories with `gleam.toml`.
The compiler should emit deterministic Wasm and optional WAT/debug artifacts.

Each example must have:

1. A checked fixture or integration test that compiles it.
2. A small host adapter checked into `examples/`.
3. Documentation for the host imports and exported functions it uses.
4. Clear diagnostics for the next unsupported feature encountered.

## Active tasks

See [Lustre weather example tasks](../tasks/18_lustre_weather_example.md) and
[Wisp static data API tasks](../tasks/19_wisp_static_data_api_example.md).
