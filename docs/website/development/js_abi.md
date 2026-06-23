# JavaScript Host ABI

Regulus targets browser-capable WebAssembly. Raw Wasm only passes scalar values,
so JavaScript hosts need a stable contract for strings, managed values, imports,
exports, and glue.

This page defines the first JS host ABI. Browser, bundler, and Node.js profiles
share these value rules. Profiles only change loading behavior, available host
APIs, and accepted import module names.

## Scope

The ABI covers values and calls crossing between compiled Gleam Wasm and a
JavaScript host. It does not define application logic, browser networking
policy, routing, response construction, or library behavior that can compile
from Gleam source.

The milestone path is intentionally small: compile a Gleam project to Wasm plus
JS glue, call a selected JavaScript external, pass a JS string into Gleam,
return a Gleam string to JS, and run the path through browser, bundler, and
Node.js profiles without handwritten pointer arithmetic in application code.

## Ownership model

Scalar values cross the boundary as raw Wasm values. Managed Gleam values cross
as borrowed `i32` pointers into guest memory.

JavaScript hosts may read managed values through exported Regulus runtime
helpers. Hosts must not mutate guest object memory. Hosts must not keep managed
pointers after a Wasm instance is discarded, reset, or after any future arena
reset point.

## Value shapes

| Gleam shape      | Wasm ABI                     | JavaScript contract                    |
| ---------------- | ---------------------------- | -------------------------------------- |
| `Int`            | `i64`                        | JS `BigInt`,                           |
| `Float`          | `f64`                        | JS `number`.                           |
| `Bool`           | `i32`                        | `0` is `false`; `1` is `true`.         |
| `Nil` return     | no result                    | JS `undefined`.                        |
| `String`         | managed `i32` pointer        | Read and write through string helpers. |
| tuples           | managed `i32` pointer        | Read through value helpers.            |
| records          | managed `i32` pointer        | Read through value helpers.            |
| custom types     | managed `i32` pointer        | Read through value helpers.            |
| lists            | managed `i32` pointer        | Read as JavaScript arrays.             |
| `Dynamic`        | managed `i32` pointer        | Read and write as JSON-shaped values.  |
| opaque externals | managed `i32` opaque pointer | JS adapter handle table entry.         |
| functions        | none                         | Unsupported across the JS host ABI.    |

The first stable call conversion layer is scalar and string focused. Managed
structured values can be read from borrowed pointers with explicit reader
shapes.

`Dynamic` is the first structured write bridge because it is the runtime representation
used for host JSON.

## Dynamic JSON bridge

Generated or packaged JS glue exposes:

- `writeDynamic(value) -> ptr`
- `writeJson(text) -> ptr`
- `readDynamic(ptr) -> value`

`writeDynamic` accepts JSON-shaped JavaScript values: `null`, booleans,
strings, finite numbers, arrays, and plain objects with string keys. Strings
remain JSON string values. Use `writeJson` when a string contains JSON source
text that should be parsed before becoming `Dynamic`.

## Opaque handles

Opaque host objects are represented by tag-8 managed objects:

```text
0..4   tag = 8
4..8   size = 0
8..12  type tag: i32
12..16 handle id: i32
```

The type tag is a stable host-defined value that distinguishes opaque external
types. The handle id indexes the JavaScript adapter's table. Guest code may pass
opaque pointers through functions and compare pointer identity, but must not
read the payload directly.

Generated or packaged JS glue exposes:

- `wrapHandle(value, typeTag = 0) -> ptr`
- `getHandle(ptr, expectedTypeTag?) -> value`
- `releaseHandle(ptr, expectedTypeTag?) -> boolean`
- `clearHandles() -> undefined`

`wrapHandle` keeps the JavaScript value alive in the adapter table and returns a
borrowed opaque pointer. `releaseHandle` drops that table reference; existing
guest pointers for the released id are invalid for future JS lookup. `init`
clears the handle table because pointers from a previous instance are invalid.
Guest memory never owns or frees the JavaScript object.

## Runtime helpers

JS host builds export stable string helpers:

- `__regulus_alloc(size: i32) -> i32`
- `__regulus_string_new(data: i32, len: i32) -> i32`
- `__regulus_string_len(ptr: i32) -> i32`
- `__regulus_string_data(ptr: i32) -> i32`

Glue writes a JS string by UTF-8 encoding it, allocating guest bytes, copying
those bytes into memory, and calling `__regulus_string_new`. Glue reads a Gleam
string by calling the length and data helpers, then UTF-8 decoding the byte
range.

Managed value reader helpers:

- `__regulus_value_tag(ptr: i32) -> i32`
- `__regulus_value_arity(ptr: i32) -> i32`
- `__regulus_value_constructor(ptr: i32) -> i32`
- `__regulus_value_field(ptr: i32, index: i32) -> i64`
- `__regulus_handle_new(type_tag: i32, handle_id: i32) -> i32`
- `__regulus_handle_type(ptr: i32) -> i32`
- `__regulus_handle_id(ptr: i32) -> i32`

`__regulus_value_tag` reads the runtime object tag. Stable tags are:

| Tag  | Runtime object |
| ---- | -------------- |
| `1`  | `String`       |
| `2`  | list cons cell |
| `3`  | tuple          |
| `4`  | record         |
| `5`  | custom type    |
| `6`  | closure        |
| `7`  | bit array      |
| `8`  | opaque value   |
| `9`  | error payload  |
| `10` | panic payload  |

`__regulus_value_arity` reads the object arity. For tuples and records this is
field count. For list cons cells it is `2`. For custom types it is constructor
field count.

`__regulus_value_constructor` reads the constructor tag for custom types. The
value is only meaningful when `__regulus_value_tag(ptr)` is `5`.

`__regulus_value_field` reads a raw field slot as a Wasm `i64`, surfaced to JS
as `BigInt`. For tuples, records, and list cons cells, field index `0` starts
after the object header. For custom, error, and panic payloads, field index `0`
starts after the constructor or reason tag.

Scalar `Int` fields are the signed `i64` value. `Bool` fields use `0n` and
`1n`. `Float` fields are the raw IEEE-754 bits reinterpreted from the `i64`
slot. Managed fields store a borrowed pointer in the low 32 bits. Glue should
convert those pointer fields before recursively reading them.

## Generated adapters

For browser, bundler, and Node.js targets, `compile` and `build` write a
deterministic sibling `.mjs` adapter whenever Wasm output is requested. The
adapter embeds compiler-generated ABI metadata for supported imports and
exports, exposes it as `abi`, and uses the same metadata to wrap host imports
and checked exported calls.

The adapter is shared across JS profiles. Profile helpers only provide loading
defaults and standard imports:

- `init(wasm, imports)` for the shared checked instantiation path
- `initBrowserPage(wasm, imports, options)` and `createBrowserImports(options)`
- `initNode(wasm, imports, options)` and `createNodeImports(options)`
- `call(name, ...args)` for metadata-driven export calls
- `callExport(name, params, result, ...args)` for explicit checked calls
- `exportFunction(name, params, result)` for reusable checked wrappers

Hosts may pass a URL, bytes, an `ArrayBufferView`, or a precompiled
`WebAssembly.Module`. Node.js hosts may also pass a relative or absolute
filesystem path or `file:` URL.

## Structured readers

Generated or packaged JS glue exposes reader helpers over borrowed managed
pointers. All readers require an explicit shape because raw runtime objects do
not store source field names or type parameters.

Tuple readers return arrays. Tuple shape items are in tuple field order:

```js
readTuple(ptr, ["Int", "String"]);
// => [1n, "text"]
```

Record readers return plain objects. Record field shapes are in constructor
declaration order:

```js
readRecord(ptr, [
  { name: "status", type: "Int" },
  { name: "body", type: "String" },
]);
// => { status: 200n, body: "ok" }
```

Custom-type readers return `{ tag, fields }`. When the shape includes variant
names, `tag` is the constructor name. Otherwise `tag` is the numeric constructor
tag. Positional variant fields return an array. Named variant fields return an
object.

```js
readCustom(ptr, {
  Created: { fields: [{ name: "id", type: "String" }] },
  Deleted: { fields: ["String"] },
});
// => { tag: "Created", fields: { id: "abc" } }
```

List readers follow tag-2 cons cells until the null pointer `0`, which is the
empty list. They return JavaScript arrays and recursively read each head with
the item shape. A list of strings uses the normal string shape:

```js
readList(ptr, "String");
// => ["a", "b", "c"]
```

`Result(a, e)` values are tag-5 custom objects with `Ok` or `Error`
constructor tags. JS readers return tagged objects:

```js
readResult(ptr, "String", "Int");
// => { tag: "Ok", value: "done" }
// => { tag: "Error", value: 404n }
```

`Option(a)` values are tag-5 custom objects with `Some` or `None` constructor
tags. JS readers return tagged objects:

```js
readOption(ptr, "String");
// => { tag: "Some", value: "found" }
// => { tag: "None" }
```

The generic `readValue(ptr, shape)` helper accepts scalar names and structured
shape objects:

```js
readValue(ptr, { kind: "List", item: "String" });
readValue(ptr, { kind: "Result", ok: "String", error: "Int" });
```

Browser, bundler, and Node.js adapters embed ABI metadata for supported
imports and exports. The metadata uses the same shape objects consumed by
`readValue`, so `call` and `exportFunction` can decode structured return
pointers without handwritten shape arrays.

## Import modules

The shared JS import namespace is `regulus/js`. Profile-specific modules are
reserved for host APIs.

| JS host profile | Accepted import modules |
| --------------- | ----------------------- |
| `bundler`       | `regulus/js`            |
| `browser`       | `regulus/js`, `browser` |
| `nodejs`        | `regulus/js`, `nodejs`  |

The `browser` module is reserved for browser APIs such as fetch, local storage,
time, and online state. The `nodejs` module is reserved for Node-specific APIs.
The compiler does not implement those APIs; host glue provides them.

Browser-profile glue exposes `createBrowserImports(options)` for the first
standard browser APIs. It returns a `browser` import module with these names:

| Import name               | Gleam ABI shape           | JavaScript behavior                                                                         |
| ------------------------- | ------------------------- | ------------------------------------------------------------------------------------------- |
| `fetch`                   | `String -> opaque handle` | Calls `fetch(url)` and stores the returned promise or response in the adapter handle table. |
| `localStorage.getItem`    | `String -> String`        | Reads a key and maps missing values to the empty string.                                    |
| `localStorage.setItem`    | `String, String -> Nil`   | Writes a string value for a key.                                                            |
| `localStorage.removeItem` | `String -> Nil`           | Removes a key.                                                                              |
| `time.now`                | `Nil -> Int`              | Returns Unix time in milliseconds.                                                          |
| `online.isOnline`         | `Nil -> Bool`             | Reads `navigator.onLine`, defaulting to `true` when unavailable.                            |

The helper accepts `fetch`, `localStorage`, `now`, and `navigator` overrides so
tests and custom browser hosts can supply checked implementations.
Browser-profile adapters also expose `initBrowserPage(wasm, imports, options)`,
which merges those standard browser imports with additional application imports
and instantiates the Wasm module.

Node-profile glue exposes `createNodeImports(options)` for the first standard
Node APIs. It returns a `nodejs` import module with these names:

| Import name | Gleam ABI shape    | JavaScript behavior                                                   |
| ----------- | ------------------ | --------------------------------------------------------------------- |
| `env.get`   | `String -> String` | Reads an environment key and maps missing values to the empty string. |
| `time.now`  | `Nil -> Int`       | Returns Unix time in milliseconds.                                    |

The helper accepts `env` and `now` overrides so tests and custom Node hosts can
supply checked implementations.

Node-profile adapters also expose `initNode(wasm, imports, options)`. The
generated adapter defaults to loading the sibling `.wasm` file resolved from
`import.meta.url`. Hosts may also pass a relative or absolute filesystem path,
a `file:` URL, bytes, or a precompiled `WebAssembly.Module`. Filesystem-backed
loads use `node:fs/promises`; after bytes are loaded, standard Node imports,
additional application imports, and exports use the same checked JS ABI
conversion as the browser and bundler profiles.

Non-JS targets use different modules and are outside this contract. Wasmtime
uses `env`, and WASI uses `wasi_snapshot_preview1`.

## Bodyless externals

Source declarations of the form
`@external(javascript, "module", "name") pub fn f(...) -> ...` are represented
as external functions with target, module, and function metadata. Project and
dependency module interfaces preserve that metadata so lowering can select the
external that matches the compile target.

Selected JavaScript externals lower through the same host-import ABI as
handwritten `external fn ... = "module" "name"` declarations. Imported
bodyless externals from project modules or dependency interfaces become
synthetic host-import functions in IR, using generated backend names for linked
modules while preserving the declared JavaScript module and function names.

If dependency source cannot be compiled and a referenced dependency member is
not represented by selected external metadata, lowering reports a
source-spanned missing-member diagnostic instead of emitting unresolved calls.
Unsupported parameter or return ABI shapes are validated before Wasm emission
for both local external declarations and imported bodyless metadata.

## Imported functions

A JavaScript host import is a Gleam `external fn` whose module is accepted by
the selected JS profile.

Supported imported parameter shapes are:

- `Int`
- `Float`
- `Bool`
- `String`
- opaque external types

Supported imported return shapes are:

- `Int`
- `Float`
- `Bool`
- `String`
- `Nil`
- opaque external types

Structured managed values may lower as borrowed pointers internally. Stable JS
conversion for imported structured parameters and returns is deferred until
structured writers are complete. Generic values and function values are
unsupported across JS host imports until their ABI contracts are defined.

## Exported functions

A public Gleam function exported to a JS host uses scalar and string
parameters. Return values may also use supported structured managed shapes.

Supported exported parameter shapes are:

- `Int`
- `Float`
- `Bool`
- `String`

Supported exported return shapes for checked call wrappers are:

- `Int`
- `Float`
- `Bool`
- `String`
- `Nil`
- tuples whose fields are supported reader shapes
- records and single-constructor record-like custom types whose fields are
  supported reader shapes
- custom types with supported field shapes and visible constructor metadata
- lists whose item type is a supported reader shape
- `List(String)`
- `Result(a, e)` when `a` and `e` are supported reader shapes
- `Option(a)` when `a` is a supported reader shape

Generated JS adapter metadata maps public export names to parameter and return
shapes. `call("name", ...args)` uses that metadata to convert scalar
parameters, invoke the Wasm export, and decode the return value.

Glue should expose checked wrappers for stable call shapes so application code
does not perform pointer arithmetic.

## Diagnostics

Unsupported JS host ABI shapes should fail before Wasm emission with
source-spanned diagnostics.

Diagnostics should cover:

- unsupported profile names
- unsupported import modules for the selected profile
- unsupported imported parameter or return shapes
- unsupported exported parameter or return shapes
- unsupported opaque-handle parameters or returns
- function or closure values crossing the JS boundary

Diagnostics should point at the external module string, parameter type, return
type, or public function annotation that caused the unsupported shape.

## Deferred contracts

This contract intentionally does not define:

- writing structured JavaScript values into Gleam
- structured import parameters or returns

Those pieces build on the scalar, string, module-name, metadata, and validation
contract above.
