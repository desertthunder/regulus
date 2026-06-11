# Patterns, visibility, and packages

Patterns test the shape of a value and introduce names for the pieces that
matched.

```gleam
case result {
  Ok(value) -> value
  Error(reason) -> reason
}
```

`Ok` and `Error` are constructor references. `value` and `reason` are new
bindings, each scoped to the branch where it appears.

Pattern bindings are not visible in other branches:

```gleam
case result {
  Ok(value) -> value
  Error(_) -> value
}
```

The `value` in the `Error` branch is unknown. It was only introduced by the
`Ok(value)` pattern.

## Pattern binding rules

Patterns can contain nested structures. A list pattern can bind the first element
and the rest of the list. A record pattern can bind labelled fields. A tuple
pattern can bind each element.

```gleam
case users {
  [first, ..rest] -> first
  [] -> "none"
}
```

```gleam
case person {
  Person(name: name, age: _) -> name
}
```

The resolver has to walk the whole pattern and define each variable binding in
the branch scope. It also has to reject duplicate bindings in one pattern:

```gleam
case pair {
  #(x, x) -> x
}
```

That pattern would give one branch scope two local symbols named `x`, so the
reference would be ambiguous.

Gleam record patterns can also use `_` and `..` to discard fields.[^1] Discarded
parts do not introduce names.

## Constructors and fields

Gleam custom types define type names and constructor names:

```gleam
pub type User {
  User(name: String, age: Int)
}
```

The type name `User` belongs in the type namespace. The constructor `User`
belongs in the constructor namespace. The fields `name` and `age` are field
names used by construction, access, update, and patterns.

Record field access depends on type information in full Gleam. The tour explains
that a field can always be accessed when all variants have a field with the same
name, position, and type; other fields require the compiler to know the specific
variant.[^2] That means a complete compiler may need type checking and name
resolution to cooperate for some field operations.

For this project, the resolver records field names as symbols so later passes
can attach type and runtime-layout information to them.

## Public and private declarations

Name resolution is also where module boundaries become visible. A module can
refer to its own private declarations, but another module should only access
public declarations.

```gleam
// in src/app/user.gleam
fn secret() {
  1
}

pub fn visible() {
  2
}
```

Another module can call `user.visible()`, but `user.secret()` should be rejected.
The resolver needs module interface data: which names a module defines, which
namespace each name belongs to, and whether each member is public.

Opaque types add a related rule. An opaque type can be public while its
constructors remain private outside the defining module.[^3] Other modules can
use the type name, but they cannot construct or pattern match on its private
representation.

## Package graphs

A single-file resolver can handle local examples, but real Gleam projects have
many modules and dependencies. A package is described by `gleam.toml`, and the
build tool compiles project modules and dependency modules together.[^4]

Cross-module name resolution needs at least three pieces of information:

- the module graph, so imports can be matched to source files or dependencies
- module interfaces, so public members can be resolved without exposing private
  implementation details
- diagnostics that point to the importing module and the referenced name

A qualified name such as `app/user.visible` is a path through the project graph
to a module interface, then to a public symbol in that interface.

[^1]: Gleam Language Tour, "Record pattern matching":
    https://tour.gleam.run/data-types/record-pattern-matching/
[^2]: Gleam Language Tour, "Record accessors":
    https://tour.gleam.run/data-types/record-accessors/
[^3]: Gleam Language Tour, "Opaque types":
    https://tour.gleam.run/advanced-features/opaque-types/
[^4]: Gleam, "Writing Gleam":
    https://gleam.run/writing-gleam/
