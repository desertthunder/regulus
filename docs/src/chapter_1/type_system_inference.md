# Gleam's type system and inference model

Gleam is statically typed. Types are checked before the program runs, and Gleam
does not have `null`, implicit conversions, exceptions, or partial type
checking.[^1]

```gleam
pub fn double(x: Int) -> Int {
  x * 2
}
```

If a function expects a `String`, passing an `Int` is a compile-time error.

```gleam
import gleam/io

pub fn main() {
  io.println(4)
}
```

`io.println` prints strings, so this call is rejected.

## Annotations and inference

Gleam can infer many local types:

```gleam
let name = "Lucy"
let count = 3
```

The compiler can tell that `name` is a `String` and `count` is an `Int` from the
right-hand side. Let bindings can have annotations when they make code clearer or
when the programmer wants the compiler to check a specific type.[^2]

```gleam
let name: String = "Lucy"
```

Function argument and return annotations are optional, but the official tour
describes them as good practice for clarity and intentional design.[^3]

```gleam
pub fn greet(name: String) -> String {
  "Hello, " <> name
}
```

For a compiler, annotations are useful boundaries. They document exported APIs,
give type checking a stable interface between modules, and make diagnostics more
direct when code does not match the intended type.

## Generic functions

Generic functions use type variables written with lowercase names.[^4]

```gleam
fn twice(argument: value, change: fn(value) -> value) -> value {
  change(change(argument))
}
```

The type variable `value` stands for one concrete type each time the function is
called. It is not an `any` type. In one call it may be `Int`; in another call it
may be `String`.

```gleam
twice(10, fn(x) { x + 1 })
twice("Hi", fn(x) { x <> "!" })
```

Each call must be internally consistent. A function that takes a `String` cannot
be used as the second argument when the first argument is an `Int`.

## Generic data

Lists are generic:

```gleam
[1, 2, 3]
["a", "b", "c"]
```

The first list has type `List(Int)`. The second has type `List(String)`.

Custom types can be generic too.[^5]

```gleam
pub type Option(inner) {
  Some(inner)
  None
}
```

The built-in `Result(ok, error)` type is a common example. It describes a
computation that can succeed with `Ok(ok)` or fail with `Error(error)`.[^6]

```gleam
fn parse_count(text: String) -> Result(Int, Nil) {
  todo
}
```

The standard library's `gleam/result` module provides helpers for composing
result values, such as `map`, `flatten`, `unwrap`, and `all`.

## Pattern and branch types

Pattern matching uses type information. If a value has type `Result(Int,
String)`, the `Ok` branch contains an `Int` and the `Error` branch contains a
`String`.

```gleam
import gleam/int

fn describe(result: Result(Int, String)) -> String {
  case result {
    Ok(number) -> int.to_string(number)
    Error(message) -> message
  }
}
```

All branches of a `case` expression must produce compatible types:

```gleam
case ready {
  True -> "yes"
  False -> "no"
}
```

This expression has type `String`. If one branch returned an `Int`, the compiler
would reject the case expression.

## Type errors as guidance

Type errors are safety checks and explanations. They tell the programmer which
part of the program does not match the language's rules. Good diagnostics point
at the expression, pattern, or annotation that caused the mismatch and describe
the expected and actual types.

Earlier compiler passes preserve source spans for this reason. A type checker
can only produce precise feedback if the AST knows where each expression and
annotation came from.

[^1]: Gleam Language Tour, "Type checking":
    https://tour.gleam.run/basics/type-checking/
[^2]: Gleam Language Tour, "Type annotations":
    https://tour.gleam.run/basics/type-annotations/
[^3]: Gleam Language Tour, "Functions":
    https://tour.gleam.run/functions/functions/
[^4]: Gleam Language Tour, "Generic functions":
    https://tour.gleam.run/functions/generic-functions/
[^5]: Gleam Language Tour, "Generic custom types":
    https://tour.gleam.run/data-types/generic-custom-types/
[^6]: `gleam/result` standard library docs:
    https://hexdocs.pm/gleam_stdlib/gleam/result.html
