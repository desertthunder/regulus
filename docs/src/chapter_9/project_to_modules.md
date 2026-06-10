# Project input and module graph

A compiler that only accepts one source string is useful for tests, but
real Gleam programs are multi-file projects. Project compilation starts by
finding package configuration and source modules before language phases run.

## Project root

Regulus treats a directory as a project root. If the user gives a file
path, the file's parent directory is used as the root for project loading.

The project loader reads `gleam.toml`. Gleam's project configuration
documents package metadata such as `name`, `version`, `description`,
`licences`, repository links, target, dependencies, and dev
dependencies.[^gleam-toml] Regulus models those fields in `GleamToml`.

## Module discovery

The loader walks:

```text
src/
test/
```

and collects `.gleam` files. Module names come from paths relative to
the source root:

```text
src/app.gleam      -> app
src/app/view.gleam -> app/view
```

Each module receives a stable `SourceFileId`. Diagnostics and later
compiler data can use that ID to refer back to the original source.

## Duplicate modules

`src/app.gleam` and `test/app.gleam` both map to module name `app`.
Regulus reports that as a project error because later phases need one
unique source file per module name.

This is an architecture issue, not only a user-experience issue. Name
resolution and imported interfaces are keyed by module names. Duplicate
names would make imports ambiguous.

## Module load order and the dependency graph

In a multi-module project, modules can import each other. Name resolution
and type checking need interfaces from imported modules before they can
check an importing module. The project model must determine a safe
evaluation order.

The safe order is a **topological sort** of the import graph: a module
comes before all modules that import it. If there is a cycle, the compiler
must either reject it or handle it specially. Gleam does not permit
circular module imports, so Regulus can report a cycle as a project error
and stop before running language phases.[^topo]

A topological sort also exposes parallelism: modules that do not import
each other can be compiled independently. Production compilers exploit
this for parallel builds. Rust's `cargo` compiles crates in parallel along
topological order, and GCC can parallelize files that have no dependencies.
Regulus does not yet implement parallel compilation, but a topological
ordering is the prerequisite for it.

## Dependencies

Regulus records dependency declarations from `gleam.toml` as package-graph
metadata. A dependency entry may be a version string or an options table
with fields such as `version`, `path`, or `git`.

The current loader records these requirements but does not yet load
dependency source or package-interface data. That is the right next
boundary: project loading should discover what packages exist, then
resolution and type checking should consume module interfaces for the
packages that are available.

## Package interface resolution

When a module imports from a dependency package, the compiler needs the
package's interface: the names it exports, the types those names have, and
the type parameters those types carry. There are two strategies for
obtaining this interface.

**Compile from source**: fetch the package source and compile it the same
way user modules are compiled. This is the approach Gleam's own compiler
takes when building for Erlang and JavaScript targets. It guarantees that
the interface is consistent with the version of the source being compiled.

**Load a pre-built interface**: read a serialized interface file produced
by an earlier compilation. This is faster and enables separate compilation,
but requires a format for interface files and a trust relationship with the
build that produced them.

Regulus currently models stdlib interfaces as hardcoded Rust data for
`gleam/io`, `gleam/int`, `gleam/string`, `gleam/list`, and others. Real
dependency compilation will require one of these strategies (or a hybrid)
at scale.

## Single-file path

The single-file compiler path remains important. Unit tests and small
examples should not need a full project. `SourceFile::new(SourceFileId(0),
source)` is enough to parse, resolve, type check, lower, and emit a small
module.

The risk is letting the single-file path become the only architecture.
Project loading exists so module IDs, imports, visibility, package
dependencies, and interfaces have a place to live.

## Incremental compilation design space

An incremental compiler avoids repeating work when only part of the project
changes. The requirements are:

1. A **stable identity** for each work unit (source file, module, function).
   Regulus already assigns stable `SourceFileId` values to modules.

2. **Tracked dependencies** between work units. Changing `app.gleam` should
   only re-check modules that import `app`, not all modules.

3. **Cached outputs** for unchanged work units. The resolved module,
   typed module, and core IR for a file that has not changed should not
   need to be recomputed.

4. An **invalidation strategy** that correctly determines what has changed.
   The simplest approach is file-content hashing. More precise approaches
   track which exported symbols changed and only invalidate dependents that
   use those symbols.

Rust's query system implements all four requirements using a demand-driven
model: computation is triggered on demand, results are memoized, and
dependents are tracked automatically.[^rustc-dev] Salsa is a standalone
library for the same pattern.[^salsa]

Regulus's current architecture satisfies requirement 1 and does not yet
implement 2–4. Adding them would not require changing the phase pipeline,
only adding caching around it.

[^gleam-toml]: Gleam, "`gleam.toml`": https://gleam.run/writing-gleam/gleam-toml/

[^topo]: Cormen et al., _Introduction to Algorithms_, "Topological sort": https://mitpress.mit.edu/9780262046305/introduction-to-algorithms/

[^rustc-dev]: Rust Compiler Development Guide, "Queries: demand-driven compilation": https://rustc-dev-guide.rust-lang.org/query.html

[^salsa]: Salsa incremental computation framework: https://salsa-rs.github.io/salsa/
