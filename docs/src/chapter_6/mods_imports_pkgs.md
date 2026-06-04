# Gleam modules, imports, and packages

A Gleam file is a module. The module name comes from its path in the project.[^1]
A file at `src/app/view.gleam` is the module `app/view`.

```text
src/app.gleam       -> app
src/app/view.gleam  -> app/view
```

Modules group definitions that belong together: functions, types, constants, and
external declarations. All Gleam code lives in a module.

## Imports

Modules can import other modules:

```gleam
import gleam/io
import gleam/string as text
```

The first import makes the final module segment, `io`, available in the file.
The second import gives `gleam/string` the local name `text`.

```gleam
pub fn main() {
  io.println(text.reverse("desserts"))
}
```

Qualified calls make the defining module visible at the call site. Readers can
see where the function comes from, and the compiler can resolve `io.println` only
after `io` is known to be an imported module.

Gleam also supports unqualified imports:

```gleam
import gleam/io.{println}

pub fn main() {
  println("Hello")
}
```

The official tour recommends qualified imports for functions because they make
code easier to read, but unqualified imports are available when they are clearer
for a specific module.[^2]

Types can be imported unqualified too:

```gleam
import gleam/string_tree.{type StringTree}
```

The `type` marker tells the compiler that `StringTree` is a type import. Gleam
types are commonly imported this way, while functions are commonly called
through the module name.[^3]

## Public and private definitions

Definitions are private to their module unless they are marked `pub`:

```gleam
fn helper(name: String) -> String {
  "Hello, " <> name
}

pub fn greet(name: String) -> String {
  helper(name)
}
```

Other modules can call `greet`, but they cannot call `helper`. This is part of
the module interface: the public surface that other modules may depend on.

Gleam also has internal modules. Public functions in modules under
`packagename/internal` can be imported by code in the package, but they are not
treated as stable public API for package users.[^4]

## Packages

A Gleam project is described by `gleam.toml`.[^5] It stores package metadata such
as the package name, version, target, licences, links, and dependencies.
Dependencies are split into normal dependencies and development dependencies.

A small package might look like this:

```toml
name = "my_app"
version = "1.0.0"
target = "javascript"

[dependencies]
gleam_stdlib = ">= 0.44.0 and < 2.0.0"

[dev-dependencies]
gleeunit = ">= 1.0.0 and < 2.0.0"
```

Source modules live under `src/`, test modules live under `test/`, and
dependencies are fetched and built by Gleam's tooling. The package manifest
connects source files to dependency packages, and the module graph gives the
compiler the set of modules it must parse, resolve, type check, and compile.

[^1]: Gleam Language Tour, "Modules":
    https://tour.gleam.run/basics/modules/
[^2]: Gleam Language Tour, "Unqualified imports":
    https://tour.gleam.run/basics/unqualified-imports/
[^3]: Gleam Language Tour, "Type imports":
    https://tour.gleam.run/basics/type-imports/
[^4]: Gleam, "Writing Gleam":
    https://gleam.run/writing-gleam/
[^5]: Gleam `gleam.toml` documentation:
    https://gleam.run/writing-gleam/gleam-toml/
