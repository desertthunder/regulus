# Pattern typing, imports, constructors, and opaque values

Type checking depends on the names found by resolution, but resolution is not
enough. A constructor name can be in scope and still be used with the wrong
field type. A public opaque type can be named by another module while its
constructors remain unavailable.

Pattern typing is the inverse of expression typing. Expression typing asks,
"what type does this expression produce?" Pattern typing asks, "given an
expected subject type, what bindings can this pattern introduce?" In ML-style
languages, the same constructor metadata supports both construction and
deconstruction.

## Pattern typing

A pattern is checked against an expected subject type:

```gleam
case pair {
  #(name, age) -> name
}
```

If `pair` has type `#(String, Int)`, the pattern binds `name: String` and
`age: Int`. If `pair` is not a tuple, the tuple pattern is invalid.

List patterns work the same way:

```gleam
case items {
  [head, ..tail] -> head
  [] -> 0
}
```

If `items` has type `List(Int)`, `head` is `Int` and `tail` is `List(Int)`.
Every element pattern is checked against the list element type.

This expected-type direction is important. The pattern `[]` alone does not say
which list element type it has. In `case items { [] -> ... }`, the subject type
supplies the missing element type.

## Constructor patterns

Custom-type constructors carry field types:

```gleam
pub type Person {
  Person(name: String, age: Int)
}
```

The constructor `Person` can be used as an expression to build a value, and as a
pattern to inspect a value:

```gleam
case person {
  Person(name:, age:) -> name
}
```

The checker reads the constructor metadata, checks that the subject type is the
constructor's return type, and then binds field names to the field types. A
labelled field pattern such as `name:` binds `name` when no nested pattern is
written.

For generic constructors, fields are instantiated from the concrete subject
type. If the subject is `Result(Int, String)`, then `Ok(value)` binds
`value: Int` and `Error(message)` binds `message: String`.

## Coverage and redundancy

Pattern matching has two common static checks:

- exhaustiveness: every possible subject value has a branch
- redundancy: a branch can never match because earlier branches cover it

Luc Maranget's work on ML pattern-match warnings frames these as the anomalies
of non-exhaustive matches and useless clauses.[^maranget] The practical
compiler question is whether the set of patterns covers the space of values
described by the subject type.

For a boolean subject, the space is small:

```gleam
case flag {
  True -> 1
  False -> 0
}
```

For a custom type, the space is the constructor set:

```gleam
pub type Status {
  Draft
  Published
  Archived
}
```

A `case Status` expression is exhaustive only if every constructor is covered or
there is a catch-all pattern. A branch after an unguarded catch-all is
redundant:

```gleam
case status {
  _ -> 0
  Published -> 1
}
```

The `Published` branch is unreachable. Regulus already implements a small
version of this analysis for booleans, lists, tuples, and custom types. A full
implementation should use the same constructor and product-field metadata but
handle nested patterns, alternatives, guards, and multiple subjects with more
precision.

## Imported values

Imports provide names, but the checker needs type information for those names.
When a project is checked, Regulus collects top-level function and constant
types from all resolved modules. It stores both unqualified names and
module-qualified names such as `app.id`.

That lets a later module check:

```gleam
import app

fn main() {
  app.id(1)
}
```

The field access syntax is resolved as a module-qualified value access, then the
type checker looks up `app.id` in the imported value map.

## Type imports

Gleam type imports let modules bring type names into scope. The tour shows the
`type` marker for importing a type without importing a value function under the
same syntax.[^type-imports]

```gleam
import gleam/string_tree.{type StringTree}
```

Resolution decides whether the imported type name is visible. Type checking
then needs the declaration behind the name: its parameters, constructors,
fields, and opacity.

## Opaque values

An opaque type can be public while hiding its constructors from other modules.
The official tour presents this as a way to expose a type while preserving
invariants through a safe API.[^opaque]

```gleam
pub opaque type PositiveInt {
  PositiveInt(Int)
}
```

Outside the defining module, another module may refer to `PositiveInt` in a type
annotation. It may not call `PositiveInt(1)` unless the constructor is exported
through the module boundary. This is a compile-time rule. The runtime
representation may be identical to a non-opaque custom type.

Regulus represents opaque declarations with `Type::Opaque` and records the
`opaque` flag in `TypeDeclaration`. Name resolution already rejects private or
opaque constructors across project modules. Type checking must preserve the
opaque type information so lowering and ABI decisions still know the value's
runtime shape.

[^type-imports]: Gleam Language Tour, "Type imports": https://tour.gleam.run/basics/type-imports/
[^opaque]: Gleam Language Tour, "Opaque types": https://tour.gleam.run/advanced-features/opaque-types/
[^maranget]: Luc Maranget, "Warnings for pattern matching": https://www.cambridge.org/core/journals/journal-of-functional-programming/article/warnings-for-pattern-matching/3165B75113781E2431E3856972940347
