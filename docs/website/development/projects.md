# Project Compilation

Regulus supports both single-file compilation and linked project builds from
`gleam.toml`. This page records the implementation contract behind project
compilation. For usage, see [Project compilation and dependencies](../reference/compiling-projects.md).

## Goal

Project compilation runs the same explicit phases as single-file compilation,
but across all selected modules in dependency order:

```text
load project -> parse -> target select -> resolve -> type check -> lower ->
link IR -> emit Wasm
```

The output is one linked Wasm module for the selected target. Debug dumps may
include per-module views, but the final artifact behaves like one compiled
project.

## Command line

Project compilation uses the high-level build command while preserving
single-file compilation as a lower-level compiler path:

```sh
reggie build [PROJECT]
reggie compile [OPTIONS] <INPUT.gleam>
```

`build` loads the current directory, a project directory, or the project that
owns a `gleam.toml` path. `compile` is currently independent from `gleam.toml`

Build and compile share flags where their meanings match:

- `-o, --output <FILE>` writes the final `.wasm` artifact to an exact path.
- `--out-dir <DIR>` writes compiler-named artifacts into a directory.
- `--target <TARGET>` selects `wasmtime`, `browser`, `bundler`, `nodejs`, or
  `wasi`.
- `--emit <KIND[,KIND]>` selects `wasm`, `wat`, `ast`, `resolved`, `typed`, or
  `ir`.
- `--dump-dir <DIR>` writes debug dumps to a separate directory.
- `-v, --verbose` prints module and dependency details.
- `--json` is reserved for future machine-readable output.

Inspection stays separate from building.

The `list` command prints discovered modules without compiling them.

## Project inputs

The build command reads enough project metadata to drive compilation:

- package name and version
- source roots
- target selection
- dependency entries
- stable source file IDs
- module names and source paths

Diagnostics should report missing project files, unreadable source files,
duplicate modules, invalid module names, and unsupported configuration with file
paths and source spans where possible.

## Module graph

Project compilation uses a deterministic module graph:

1. Discover selected project modules under source roots.
2. Parse every selected module.
3. Apply target-group filtering before name resolution.
4. Resolve imports against project modules, dependency interfaces, stdlib
   interfaces, and the prelude.
5. Type-check modules in dependency order.
6. Lower typed modules to IR.
7. Link IR declarations into one backend module.

Same-project calls must not become host imports. Unknown dependency calls should
fail before lowering or byte emission.

## Dependencies

Dependency support has two layers.

Interface-only dependencies load dependency metadata and module interfaces
without compiling dependency source. This supports type checking and clear
diagnostics for packages whose behavior is behind intrinsics or host adapters.

Source dependencies load selected Hex and path dependency source modules and
compile them through the same pipeline as project modules when they fit the
supported compiler subset.

Hex loading uses the public Hex repository API used by Gleam:

- package metadata: `GET https://repo.hex.pm/packages/{name}`
- package tarball: `GET https://repo.hex.pm/tarballs/{name}-{version}.tar`
- optional full version index: `GET https://repo.hex.pm/versions`

Repository metadata responses are gzip-compressed signed protobuf payloads and
must be verified before use. Tarballs are checksum-verified from lock metadata,
then cached by checksum under:

```text
$HOME/.regulus/store/hex/tarballs/{outer-checksum}.tar
```

The global registry cache is immutable and shared across projects. Project
builds extract packages into disposable project-local build directories:

```text
{project}/build/packages/packages.toml
{project}/build/packages/{package-name}/
```

The outer tarball contains `contents.tar.gz`, which holds source files,
`gleam.toml`, and package metadata. The compiler extracts that inner archive
into the project package directory. A stamp file records name, version,
checksum, source, and cache schema version. If the stamp does not match, the
package directory is replaced.

Path dependencies are loaded from their declared path without network access.
Quiet dependency progress stays stable; `--verbose` may print source URLs,
checksums, cache paths, package build paths, and source paths.

## Package-owned interfaces

Dependency interfaces are stored as `InterfaceEntry` values containing package,
module, and interface data. Resolution attaches that owner to imports and
imported symbols so lowering and linking use exact `package:module.member`
identities instead of inferring package ownership from module names.

Unsupported dependency members, modules, syntax, runtime primitives, or ABI
shapes should produce source-spanned diagnostics before backend emission.

## Linking

The linker combines lowered project and compiled dependency modules into one
backend module. It should:

- preserve public export decisions
- rewrite same-project calls to direct or indirect internal calls
- keep dependency interface calls distinct from host imports in debug dumps
- reject duplicate generated names deterministically
- preserve source spans for linked declarations and diagnostics

## Generated names

Linked modules use deterministic generated backend names. The compiler keeps
three name classes separate:

- source names, which users wrote and diagnostics should show
- backend names, which are compiler-owned and collision-free in linked IR
- ABI names, which are intentional Wasm export or host import names

Backend names are structured data before rendering. They include owner
identity, item kind, helper kind, optional source member name, and optional
compiler-generated index.

Rendered backend names use one central renderer. Each component is escaped with
a collision-free encoding rather than punctuation replacement, so `foo-bar`,
`foo_bar`, and `foo/bar` remain distinct.

The scheme covers project declarations, lifted functions, runtime and stdlib
helpers, selected dependency package members, compiler-owned import wrappers,
and public exports. Host import ABI pairs such as module `env` and name
`clock_now` are not mangled.

Debug dumps show both source names and generated names so contributors can
trace linked calls without treating same-project members as host imports.

## Artifacts

Project compilation produces deterministic artifact names. With `-o`, the
final `.wasm` path is exact. With `--out-dir`, names are derived from the
package and artifact kind. Without either flag, the default output path is
`build/<package>.wasm`.

Supported artifacts include:

- final project `.wasm`
- optional project `.wat`
- optional per-module AST, resolved, and typed dumps
- optional linked IR dump
- bundler `.mjs` adapter output when Wasm is requested for `--target bundler`

Failed compilation should not leave partial final artifacts unless the user
explicitly requested debug dumps for completed phases.
