# JS host ABI

Regulus targets browser-capable Wasm. Raw Wasm only passes scalar values, so
JavaScript hosts need a stable ABI for strings, managed values, host APIs, and
generated or handwritten JS glue.

This spec defines the shared JavaScript host boundary. Browser, bundler, and
Node.js profiles should build on this boundary instead of adding
product-specific compiler behavior.

The JS host ABI is the next usability milestone for Regulus. Broad stdlib
coverage should not block this work. A small, documented JS boundary is more
important than reimplementing library behavior in the compiler.

## Priority

Regulus should first prove that a Gleam project can compile to Wasm plus JS
glue, call a JS external, pass a JS string into Gleam, return a Gleam string to
JS, and run through a bundler-oriented smoke test. Browser and Node.js profiles
should reuse the same value ABI after that path works.

Stdlib and dependency support should use this interop substrate wherever
possible. Runtime intrinsics remain appropriate only for compiler-owned value
representations, allocation, closure dispatch, structural equality, failure
payloads, and narrow primitives that cannot be expressed in Gleam source.

## Scope

The JS host ABI covers values and calls crossing between compiled Gleam Wasm and
a JavaScript host. It does not define application logic, browser networking
policy, routing, response construction semantics, or library behavior that can
compile from Gleam source.

The ABI should cover:

- writing JS strings into guest memory
- reading Gleam strings from managed pointers
- reading common managed values from guest memory
- passing scalar values directly through Wasm parameters and results
- passing lists, tuples, records, custom types, `Result`, and `Option`
- passing opaque JS handles when copying values is not appropriate
- naming imports and exports for browser, bundler, and Node.js profiles
- generated or documented JS glue for common host calls

## Relationship to the core host ABI

The core host ABI defines the low-level Wasm shapes: scalars are raw Wasm
values, and managed Gleam values are borrowed pointers into guest memory. The JS
host ABI is a higher-level profile over that contract for JavaScript hosts.

Hosts may inspect guest-managed values through exported runtime helpers. Hosts
must not mutate guest object memory or retain pointers across instance reset or
any future arena reset.

## JavaScript glue

Browser-capable Wasm needs JS glue comparable in role to Rust's
`wasm-bindgen`, even if Regulus starts with a smaller handwritten adapter. The
glue should hide pointer arithmetic from example code and provide explicit
helpers for common shapes.

The first glue layer can be small and stable:

- load a bundler-oriented ES module wrapper for a `.wasm` artifact
- instantiate a module with target-specific imports
- write JS strings and receive managed string pointers
- read managed strings from exported function results
- read tagged managed values for simple records, tuples, lists, and custom
  types
- call exported Gleam functions with checked argument conversion

Later glue can add generated bindings from compiler metadata.

## Structured values

Structured data crossing the JS boundary should use one of two mechanisms:

1. Copy values into or out of the guest runtime representation.
2. Pass opaque JS handles managed by the host profile.

Copied values are appropriate for strings, lists of strings, small records,
tuples, `Result`, `Option`, and simple custom types. Opaque handles are better
for host objects such as `Request`, `Response`, streams, DOM nodes, timers,
promises, file handles, and sockets.

The ABI must define how the host reads tags, fields, arity, string data, list
contents, and record/custom-type metadata. Unsupported shapes should fail before
byte emission with source-spanned diagnostics.

## Opaque JS handles

Opaque JS handles use normal managed runtime objects with tag `8`. The object
header size word is `0`. Payload word `0` is a stable `i32` type tag owned by
the host profile or package adapter. Payload word `1` is an `i32` adapter-table
handle id. Guest code may pass opaque pointers around and compare identity, but
must not read the payload directly.

The JS adapter owns the handle table. Wrapping a JS value allocates one table id
and one tag-8 guest object. Releasing a handle removes the JS table entry; any
existing guest pointer for that id is then invalid for JS lookup. Reinitializing
a Wasm instance clears the table because all managed pointers from the previous
instance are invalid.

Opaque pointers are borrowed across calls. The adapter must keep a wrapped JS
value alive until `releaseHandle`, `clearHandles`, or instance reinitialization.
Guest memory never owns, copies, or frees the JavaScript object itself.

## JS host profiles

The shared ABI is independent of the JavaScript engine. Profiles describe
module loading, generated glue shape, available host APIs, and import module
names.

The initial profiles should mirror the common `wasm-bindgen` deployment modes:

- `browser`: direct use from a browser page without a bundler
- `bundler`: ES module glue intended for Vite, Rollup, Webpack, or other
  toolchains that package JavaScript and Wasm together
- `nodejs`: Node.js loading and host APIs

Additional JavaScript environments should start from one of these profiles
unless they require a distinct module format, loading model, or host API set.
Those differences should not change the core value ABI.

## Browser profile

The browser profile reserves the `browser` import module for browser APIs used
by examples. The first stable names are:

| Import module | Import name               | Gleam ABI shape             | JavaScript behavior |
| ------------- | ------------------------- | --------------------------- | ------------------- |
| `browser`     | `fetch`                   | `String -> opaque handle`   | Calls `fetch(url)` and stores the returned promise or response in the adapter handle table. |
| `browser`     | `localStorage.getItem`    | `String -> String`          | Reads a key and maps missing values to the empty string. |
| `browser`     | `localStorage.setItem`    | `String, String -> Nil`     | Writes a string value for a key. |
| `browser`     | `localStorage.removeItem` | `String -> Nil`             | Removes a key. |
| `browser`     | `time.now`                | `Nil -> Int`                | Returns Unix time in milliseconds. |
| `browser`     | `online.isOnline`         | `Nil -> Bool`               | Reads `navigator.onLine`, defaulting to `true` when unavailable. |

These imports are ordinary Gleam external functions with target validation and
ABI checks.

The compiler should not implement browser API semantics. The JS host profile
owns the real browser calls and adapts values through the JS host ABI.
Browser glue exposes `initBrowserPage(wasm, imports, options)`, which
instantiates the Wasm module with checked browser imports from
`createBrowserImports(options)` and any additional application imports.

## Bundler profile

The bundler profile should emit or document ES module glue suitable for modern
JS toolchains. It should avoid assumptions about global script loading and
should expose imports and exports in a shape that bundlers can tree-shake.

Server-side JS adapters should start here when they use bundled ES modules.
Request and response objects may cross the boundary as copied data or opaque
handles as the ABI matures, but route logic and response shaping should remain
compiled Gleam code where possible.

## Node.js profile

The Node.js profile should define loading, filesystem access for `.wasm` files,
and Node-specific host imports. It should use the same value ABI as browser and
bundler profiles.

Node support is useful for CLI smoke tests, local examples, and host-side tests
that should not require a browser.

Node glue is emitted as an ES module next to the generated `.wasm` file. The
default Wasm location is the sibling `.wasm` URL resolved from
`import.meta.url`. The adapter exposes `initNode(wasm, imports, options)`,
which accepts the default sibling artifact, a relative or absolute filesystem
path, a `file:` URL, bytes, or a precompiled `WebAssembly.Module`.
File-backed loads use `node:fs/promises` and then instantiate with the same
checked import wrapping and export helpers as the browser and bundler profiles.

The first stable Node-specific imports are:

| Import module | Import name | Gleam ABI shape | JavaScript behavior |
| ------------- | ----------- | --------------- | ------------------- |
| `nodejs`      | `env.get`   | `String -> String` | Reads an environment key and maps missing values to the empty string. |
| `nodejs`      | `time.now`  | `Nil -> Int` | Returns Unix time in milliseconds. |

Node glue exposes `createNodeImports(options)` for those APIs. `initNode`
merges those standard imports with any additional application imports before
calling the shared checked JS ABI instantiation path.

## Diagnostics

Unsupported JS host imports, exports, value shapes, opaque handle uses, target
profiles, and glue-generation requests should produce source-spanned diagnostics
before Wasm emission.

## Bodyless externals

Source declarations of the form
`@external(javascript, "module", "name") pub fn f(...) -> ...` are represented
as external functions with target, module, and function metadata. Project and
dependency module interfaces preserve that metadata so a later lowering phase
can select the external that matches the compile target.

Selected JavaScript externals lower through the same host-import ABI as
handwritten `external fn ... = "module" "name"` declarations. Imported
bodyless externals from project modules or dependency interfaces become
synthetic host-import functions in IR, using generated backend names for linked
modules while preserving the declared JavaScript module and function names.

If dependency source cannot be compiled and a referenced dependency member is
not represented by selected external metadata, lowering reports a source-spanned
missing-member diagnostic instead of emitting unresolved calls. Unsupported
parameter or return ABI shapes are validated before Wasm emission for both
local external declarations and imported bodyless external metadata.

## Active tasks

See [JS host ABI tasks](../tasks/15_js_host_abi.md).
