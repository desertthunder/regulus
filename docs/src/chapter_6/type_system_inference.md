# Gleam's type system and inference model

Gleam is statically typed.[^1] Types are checked before the program runs. The
programmer often writes function types:

```gleam
pub fn double(x: Int) -> Int {
  x * 2
}
```

Local values can usually be inferred:

```gleam
let name = "Lucy"
```

Gleam can tell that `name` is a `String` from the value on the right-hand side.
This keeps code light without giving up static type checking.

Gleam also has generic types. A list of integers has type `List(Int)`, while a
list of strings has type `List(String)`. The shape is the same, but the element
type changes.

```gleam
[1, 2, 3]
["a", "b", "c"]
```

Generic custom types work the same way. A common example is `Result(ok, error)`,
which can represent success or failure while preserving the types of both cases.

```gleam
pub type Result(ok, error) {
  Ok(ok)
  Error(error)
}
```

The type system is one of Gleam's main strengths: it catches mistakes early while
letting many local annotations be inferred.

[^1]: Gleam language tour: https://github.com/gleam-lang/language-tour

<!--
TODO (research):
  - Gleam's official type-system docs and language-tour examples
  - Where Gleam requires type annotations and where it infers types
  - Generic function syntax and examples
  - The `Result(ok, error)` and `List(element)` types in real Gleam code
  - How Gleam reports type errors for calls, records, and pattern matching
-->
