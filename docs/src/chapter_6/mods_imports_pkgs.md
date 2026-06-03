# Gleam modules, imports, and packages

A Gleam file is a module. The module name comes from its path in the project.[^1]
A file at `src/app/view.gleam` is the module `app/view`.

```text
src/app.gleam       -> app
src/app/view.gleam  -> app/view
```

Modules can import other modules:

```gleam
import gleam/io
import gleam/list as list
```

The first import makes the final module segment, `io`, available in the file.
The second import gives `gleam/list` the local name `list`.

A Gleam project is described by `gleam.toml`.[^2] It stores package metadata such
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

Packages give Gleam code a predictable structure. Source modules live under
`src/`, test modules live under `test/`, and dependencies are fetched and built
by Gleam's tooling.

[^1]: Gleam language tour: https://github.com/gleam-lang/language-tour
[^2]: Gleam `gleam.toml` documentation: https://gleam.run/writing-gleam/gleam-toml/
