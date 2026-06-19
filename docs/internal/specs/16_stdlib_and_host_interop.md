# Standard library and host interop

The backend has a raw scalar and managed-pointer ABI for current tests. This
spec tracks the remaining work needed to compile useful upstream
`gleam_stdlib` source, keep host interop explicit, and remove bespoke stdlib
behavior over time.

The stdlib package is a Hex dependency, but it is not just ordinary Gleam source
for this compiler yet. Upstream stdlib also relies on target-specific
declarations, bodyless runtime types, JavaScript and Erlang shims, native
collection representations, dynamic values, and bit-string/text/binary
primitives. Those shapes must be represented directly before Regulus can treat
the package as another dependency.

The registry remains useful only as a transitional interface and strategy
table. It should bootstrap common programs, describe compiler-owned primitives,
and produce clear diagnostics while upstream source support matures. The end
state is no stdlib registry: stdlib interfaces come from dependency metadata or
compiled package source, and native behavior is represented by ordinary
externals, package assets, or named runtime primitives.

## Strategy

Every stdlib member exposed by the compiler should declare one strategy:

- compiled upstream Gleam source
- temporary interface only
- compiler or runtime intrinsic
- validated stdlib host shim
- adapter around a target host import

The preferred migration path is to compile upstream source first, then delete
temporary registry behavior module by module. Intrinsics and shims are reserved
for language/runtime primitives or target facilities that normal source cannot
express. Once those primitives are represented directly, the registry should be
removed rather than kept as a compatibility layer.

Unsupported stdlib code should identify the blocker category:

- source language feature
- target selection
- dependency package asset
- runtime primitive
- host or JS ABI shape

## Upstream compatibility slices

The following slices cover the known upstream stdlib shapes that are not fully
handled yet.

### Source compile readiness

Regulus should be able to compile selected modules from the published
`gleam_stdlib` package as fixtures. The first fixtures should use modules with
few native dependencies, such as `gleam/pair`, then expand through pure portions
of `gleam/order`, `gleam/result`, `gleam/option`, `gleam/list`, `gleam/int`,
and `gleam/float`.

Each fixture should report why compilation stops instead of failing later in
Wasm assembly. The report should group blockers by module and category so the
registry can be deleted deliberately.

### Target attributes

Upstream stdlib uses standalone `@target(erlang)` and `@target(javascript)`
attributes on declarations. Regulus already has target groups for externals,
but upstream stdlib also needs declaration filtering for functions, constants,
types, and externals that share names across targets.

For JS profiles, upstream `javascript` should select browser, bundler, and
Node.js builds unless a narrower Regulus target rule is added later.

### Bodyless runtime types

Bodyless runtime types such as `Dynamic`, `Dict`, and `StringTree` are ordinary
interfaces for values implemented by the runtime or host shims. Regulus should
preserve them as external type interfaces and require a declared strategy for
operations that create, inspect, or transform those values.

### The `anything` type

Upstream stdlib uses `anything` for native dynamic and inspection boundaries.
Regulus currently treats that shape like a generic type, which makes lowering
fail without monomorphization. The compiler needs an explicit representation
for `anything` as an opaque dynamic boundary type.

Initial support should be narrow. It may appear in stdlib-native externals such
as dynamic casts, dynamic indexes, and `string.inspect`. User-facing exports or
general host ABI positions using `anything` should remain unsupported until the
ABI semantics are defined.

### Native stdlib shims

Upstream JavaScript externals refer to stdlib package assets such as
`../gleam_stdlib.mjs` and `../dict.mjs`. Regulus should either package those
validated stdlib-relative shims or map them to equivalent compiler/runtime
helpers. These paths are dependency assets, not arbitrary application imports.

The compiler should preserve source module names in diagnostics and metadata
even when it maps an upstream shim to a Regulus helper.

### Dict and native collections

`gleam/dict` depends on a native dictionary representation and JavaScript HAMT
and transient dictionary helpers. Regulus can keep a registry-backed dict
surface as a temporary strategy, but upstream source support requires a real
runtime representation or a validated `dict.mjs` shim.

The dict strategy must define equality and hashing behavior, callback ABI for
fold/map-style operations, transient mutation boundaries, and how dict values
cross the JS host ABI.

### Dynamic and decoding

`gleam/dynamic` and `gleam/dynamic/decode` should compile as much Gleam source
as possible. The compiler/runtime owns only the dynamic representation, bridge
from host JSON or JSON text, primitive classification, lookup, traversal,
construction, and any primitive `DecodeError` support required by source.

Decoder combinators such as `field`, `map`, `then`, `one_of`, and `recursive`
should use normal compiled closure dispatch. Runtime-specific decoder behavior
should be treated as a temporary blocker, not a permanent stdlib strategy.

The bridge must map JSON values consistently:

| JSON shape | Dynamic shape                                                 |
| ---------- | ------------------------------------------------------------- |
| `null`     | dynamic nil/null                                              |
| boolean    | dynamic bool                                                  |
| number     | dynamic int or float, preserving integer values when possible |
| string     | dynamic string                                                |
| array      | dynamic indexed sequence of dynamic values                    |
| object     | dynamic properties with string keys                           |

### Text, binary, and URI primitives

`gleam/string`, `gleam/string_tree`, `gleam/bytes_tree`,
`gleam/bit_array`, and `gleam/uri` require native primitives that are larger
than the current group-1 string/list helpers. The missing surface includes
Unicode codepoint and grapheme operations, string slicing, byte slicing,
base16/base64 encoding, percent encode/decode, URI parsing, iodata-style byte
and string trees, bit-array concat/slice, and bit-string segment semantics.

Where upstream source delegates to native helpers, Regulus should choose either
validated stdlib shims or small runtime primitives with the same behavior.

### Bit-string semantics

The current bit-array support is useful for tests but is not complete Gleam
bit-string semantics. Upstream `gleam/bit_array` needs sized segment matching,
binary construction rules, byte alignment checks, and diagnostics that name the
unsupported segment form.

## Host ABI

The host ABI defines how values cross the Wasm boundary:

- scalar values
- strings and bit arrays
- lists, tuples, records, and custom values
- functions and closures
- dynamic values and opaque native values
- errors and panics
- memory ownership

Concrete low-level host ABI uses raw Wasm values plus runtime adapters:

| Gleam shape    | Wasm shape | Host rule                          |
| -------------- | ---------- | ---------------------------------- |
| `Int`          | `i64`      | signed 64-bit integer              |
| `Float`        | `f64`      | IEEE-754 double                    |
| `Bool`         | `i32`      | `0` false, non-zero true           |
| `Nil`          | no result  | unit value                         |
| managed values | `i32`      | borrowed pointer into guest memory |

Managed values include strings, bit arrays, lists, tuples, records, custom
values, functions, errors, panics, dynamic values, and opaque native handles.
The guest runtime owns these values. Hosts may read them while the instance is
alive, but must not mutate object memory or retain pointers across instance
reset or any future arena reset.

The runtime exports adapter helpers for low-level hosts:

| Export                                  | Purpose                              |
| --------------------------------------- | ------------------------------------ |
| `memory`                                | guest linear memory                  |
| `__regulus_string_len(ptr)`             | byte length for a string pointer     |
| `__regulus_string_data(ptr)`            | data address for a string pointer    |
| `__regulus_value_tag(ptr)`              | runtime object tag, `0` for nil/null |
| `__regulus_value_size(ptr)`             | runtime object size/arity/bit length |
| `__regulus_value_field_i64(ptr, index)` | raw managed field slot               |
| `__regulus_value_field_i32(ptr, index)` | raw field slot narrowed to `i32`     |

Compiler code receives host-provided managed values as borrowed `i32` pointers.
The host must only pass pointers allocated in the same guest memory or adapter
values with an explicit ownership rule.

Current stdlib host imports are:

| Gleam member       | Wasmtime import    | Browser import         | WASI        |
| ------------------ | ------------------ | ---------------------- | ----------- |
| `gleam/io.print`   | `env.print(ptr)`   | `browser.print(ptr)`   | unsupported |
| `gleam/io.println` | `env.println(ptr)` | `browser.println(ptr)` | unsupported |

WASI `gleam/io` is deliberately unsupported until a concrete `fd_write`
adapter is added. Unsupported host calls, target combinations, and ABI shapes
produce source-spanned diagnostics before WAT assembly.

## External functions

General Gleam external functions lower to Wasm imports. The compiler preserves
the declared external module and function name, validates that the selected
target accepts that module, and rejects unsupported ABI shapes before byte
emission.

Stdlib package shims are a constrained extension of this rule. A dependency may
reference known package assets, but those imports must be validated and either
packaged into JS output or mapped to compiler/runtime helpers. User modules
should not gain arbitrary relative JS imports through this path.

Browser examples need imports for fetch, local storage, and browser state.
JS-hosted server examples need imports or exported wrappers for request routing
and response construction. These are ordinary target-specific external
functions with documented adapters.

## JS host ABI

JavaScript hosts need a stable boundary over the low-level managed-pointer ABI.
The shared value rules, browser, bundler, and Node.js profiles, opaque handles,
and JS glue are defined in
[JavaScript host ABI](../../website/development/js_abi.md).

## Higher-order intrinsics and runtime callbacks

Compiler/runtime intrinsics must support the same closure semantics as ordinary
Gleam code. The shared callback ABI, lowering rule, reuse requirements, and
callback-taking stdlib members are defined in
[Closures and intrinsic callbacks](../../website/reference/closures.md).

Unsupported callback shapes should be rejected before WAT assembly with a
source-spanned diagnostic naming the intrinsic, closure type, and ABI shape.

## Runtime scope

The runtime is part of the compiler distribution when it supports compiled
Gleam semantics or the host ABI. It may own allocation, managed value layout,
strings, lists, records, custom values, closures, equality, debug formatting,
panic values, dynamic primitives, opaque native handles, and adapter helpers.

The runtime should not own application or library behavior that can be compiled
from Gleam source. Networking policy, routing, response construction, JSON
decoder combinator semantics, URI business rules, and product-specific data
shaping should stay in user or dependency modules unless a narrow primitive is
required by the ABI.

## Diagnostics

Unsupported stdlib modules, dependency modules, package assets, host calls, ABI
shapes, or target combinations should produce clear diagnostics rather than
failing during Wasm assembly. Diagnostics should say whether the missing piece
is source syntax, target filtering, dependency packaging, a runtime primitive,
or a host ABI rule.

## End state

The completed design has no stdlib-specific registry. `gleam_stdlib` is loaded
like a dependency package, selected source declarations are compiled normally,
bodyless native types are dependency interfaces, and target shims are validated
package assets or explicit runtime primitives. Any remaining stdlib-specific
table is a bug unless it only names compiler-owned primitives that are also
available through the normal external/runtime mechanism.

## Active tasks

See [Stdlib and host interop tasks](../tasks/16_stdlib_and_host_interop.md).
