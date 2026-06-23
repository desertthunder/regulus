# Standard Library And Host Interop

`gleam_stdlib` is an ordinary Hex package, not compiler-owned source. Regulus
stdlib support means implementing enough Gleam language, package, target, ABI,
and runtime semantics that the published package can compile as a dependency.
The compiler should not reimplement library behavior that can be compiled from
upstream source.

Current upstream package references:

- Hex package: <https://hex.pm/packages/gleam_stdlib>
- API docs: <https://gleam-stdlib.hexdocs.pm/>

## Direction

The end state is dependency-first:

- interfaces come from package metadata or compiled source
- target selection follows upstream Gleam declarations
- package JS assets are validated dependency assets
- runtime helpers cover language and ABI primitives only
- host imports are explicit target adapters
- no bespoke stdlib registry remains

The existing registry is transitional. It may bootstrap early examples and
produce diagnostics, but each entry should have a deletion path. A registry
entry that permanently duplicates upstream library behavior is a design bug.

## Ownership

Regulus owns compiler and runtime semantics:

- parsing, resolving, typing, lowering, and codegen for Gleam source
- dependency loading from Hex caches and local package paths
- `@target` filtering for declarations selected by the build target
- bodyless external type interfaces
- `anything` and dynamic boundary representation
- bit-string construction and pattern semantics
- managed value layout, allocation, equality, closures, panics, and errors
- low-level Wasm ABI and JS host ABI adapters
- validation for external imports, package assets, and host profiles

`gleam_stdlib` owns library behavior:

- collection APIs and combinators
- decoder combinator semantics
- URI parsing policy
- string, bit-array, tree, dict, set, option, result, pair, order, bool,
  int, float, and function module behavior when expressible as source

When upstream source delegates to native code, Regulus should expose the
missing primitive narrowly, package the validated dependency asset, or reject
the unsupported shape with a source-spanned diagnostic.

The scalar source migration uses private `__regulus_native` helpers for
upstream bodyless externals such as `int.to_string` and `float.to_string`.
These helpers keep the public function body in compiled dependency source
while routing only the representation-level conversion to the runtime.

## Package Source

The compiler should compile the published `gleam_stdlib` package from source as
fixtures. Start with modules with minimal native dependencies, then expand by
blocker category:

- source syntax or semantics not implemented
- target attributes and declaration filtering
- dependency metadata or package asset loading
- runtime primitive missing
- external ABI or host profile mismatch

Blocker reports should be grouped by package module and should point to the
first missing compiler capability. The report is a migration tool for deleting
registry entries, not a justification for more registry behavior.

## Monomorphized Dependency Emission

Dependency source may define public generic functions that are imported by
project code. Regulus should not export those generic dependency declarations
through the host ABI. Instead, Wasm emission should start from reachable project
exports, discover dependency calls, and emit internal concrete specializations.

A specialization is identified by:

- dependency package
- module
- function
- instantiated parameter and return types

For each reachable specialization, the compiler substitutes generic type
variables in the dependency body and interface with the concrete call types,
lowers that concrete body, assigns a deterministic internal backend name, and
rewrites calls to the specialized name. The name should preserve enough package,
module, and function identity for debug dumps while avoiding collisions between
different type instantiations.

Specialized dependency functions are implementation details. They should remain
internal, even when the upstream dependency function is public. Host ABI
validation applies to the project export surface and explicit externals, not to
generic dependency declarations that become internal specializations.

If specialization reaches an unsupported type, closure, native external, or
host ABI shape, diagnostics should point at the source call or dependency
function span that forced the unsupported specialization.

## Target Selection

Upstream stdlib uses target-specific declarations. Regulus should preserve and
apply target attributes before resolution and type checking so duplicate names
across targets do not conflict.

For Regulus JS-host profiles, upstream `javascript` declarations are selected
for browser, bundler, and Node.js builds unless a narrower rule is specified by
the package or adapter policy. Erlang declarations are not compiled for Wasm
targets unless explicitly mapped to a Regulus runtime primitive.

## Native Boundaries

Some stdlib declarations are interfaces to native target behavior. These should
be modeled as normal dependency interfaces and validated externals:

- bodyless types such as `Dynamic`, `Dict`, and `StringTree`
- `anything` at dynamic and inspection boundaries
- JS package assets such as `../gleam_stdlib.mjs` and `../dict.mjs`
- target host calls such as `gleam/io.print` and `gleam/io.println`

Package-relative JS assets are allowed only when they belong to a loaded
dependency and pass validation. User modules should not gain arbitrary relative
JS imports through the stdlib path.

Native dict values use the runtime-managed `Dict` custom value. The current
primitive keeps persistent update semantics and treats transient conversion as
an internal stdlib-native optimization boundary: transient insert and delete
consume a transient-shaped value and return the updated dict value. Dict keys
and values compare structurally through the normal managed-value equality
helper. The native hash rule is collision-only for now, so correctness comes
from structural equality rather than bucket partitioning.

For JS-family targets, validated upstream `../dict.mjs` externals in
`gleam_stdlib` are not emitted as ordinary JS host imports when a Regulus
native primitive exists for the same operation. Diagnostics and package asset
validation still preserve the upstream module and function names. General user
JS imports do not get this treatment.

## Runtime Primitives

Runtime primitives should be small and language-shaped. Valid examples include:

- allocation and managed value layout
- string and bit-array storage primitives
- closure invocation and callback ABI
- equality and ordering primitives required by language semantics
- dynamic value representation and host JSON bridges
- opaque native handles for validated host or package assets
- panic and error payload representation

Invalid examples include permanent reimplementations of decoder combinators,
URI rules, routing, collection APIs that upstream source can express, or
application-specific behavior.

## Host ABI

The host ABI defines how values cross Wasm boundaries:

| Gleam shape    | Wasm shape | Rule                               |
| -------------- | ---------- | ---------------------------------- |
| `Int`          | `i64`      | signed 64-bit integer              |
| `Float`        | `f64`      | IEEE-754 double                    |
| `Bool`         | `i32`      | `0` false, non-zero true           |
| `Nil`          | no result  | unit value                         |
| managed values | `i32`      | borrowed pointer into guest memory |

Managed values include strings, bit arrays, lists, tuples, records, custom
values, functions, dynamic values, errors, panics, and opaque handles. Hosts
may read borrowed values while the instance is alive, but must not mutate
object memory or retain pointers across resets.

The JS host ABI, browser, bundler, and Node.js profiles are defined in
[JavaScript host ABI](../../website/development/js_abi.md).

For JS-family hosts, `Dynamic` is a dedicated ABI shape rather than an opaque
handle. Adapter metadata names it as `Dynamic`; `writeDynamic` accepts host JSON
values, `writeJson` parses JSON text before conversion, and `readDynamic`
returns JSON-shaped JavaScript values. JSON arrays become dynamic arrays, JSON
objects become dynamic properties with string keys, and JSON `null` becomes
dynamic nil. Strings passed to `writeDynamic` remain JSON string values; callers
use `writeJson` when a string contains JSON source text.

## Externals And Package Assets

General external functions lower to Wasm imports. Regulus preserves declared
module and function names, validates that the selected target accepts them, and
rejects unsupported ABI shapes before byte emission.

Stdlib package assets use the same mechanism plus dependency-asset validation.
If an upstream asset is mapped to a Regulus runtime helper internally,
diagnostics and metadata should still name the upstream external declaration.

## Diagnostics

Unsupported stdlib source should fail early with source-spanned diagnostics.
Diagnostics should identify whether the missing piece is source syntax,
target selection, dependency packaging, a package asset, a runtime primitive,
or a host ABI rule.

## End State

`gleam_stdlib` is loaded and compiled like any other Hex dependency. The
compiler has no stdlib interface registry. Remaining compiler tables name
language/runtime primitives, host adapters, or validated external ABI rules,
not standard-library modules.

## Active Tasks

See [Stdlib and host interop tasks](../tasks/16_stdlib_and_host_interop.md).
