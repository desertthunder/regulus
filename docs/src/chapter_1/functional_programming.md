# Functional programming in Gleam

Gleam is a functional language where values are immutable and functions are the
main way to organize behavior.[^1] That affects how Gleam programs are written
and how a compiler represents them.

In an imperative language, a loop might update a variable again and again. Gleam
code often builds a new value from an old one:

```gleam
let name = "Lucy"
let greeting = "Hello, " <> name
greeting
```

The name `greeting` is bound to a value. The value does not change. If the same
variable name is reused later, it is a new binding that shadows the old one; the
old value has not been mutated.[^2]

## Expressions

Gleam is expression-oriented. Blocks and function bodies evaluate expressions in
order, and the final expression is the result.[^3]

```gleam
pub fn greeting(name: String) -> String {
  let prefix = "Hello, "
  prefix <> name
}
```

There is no `return` keyword in the ordinary function body. The compiler can
treat the body as an expression tree whose final value becomes the function
result.

## Functions as values

Functions are values in Gleam. They can be assigned to variables, passed to other
functions, and returned like other values.[^4]

```gleam
fn twice(value: Int, change: fn(Int) -> Int) -> Int {
  change(change(value))
}

pub fn main() {
  twice(1, fn(x) { x + 1 })
}
```

The anonymous function `fn(x) { x + 1 }` can capture names from the scope where
it is defined, making it a closure.[^5] For a compiler, that means a function
value may need a code pointer plus the captured values that the function body
can use later.

## Pipelines

Functional code often transforms data through a series of small functions. The
pipe operator, `|>`, sends the value on the left into the function call on the
right.[^6]

```gleam
import gleam/int
import gleam/string

pub fn label(number: Int) -> String {
  number
  |> int.to_string
  |> string.append(" items")
}
```

The pipeline is ordinary function application with syntax that makes the flow of
data easier to read from top to bottom.

## Recursion and lists

Gleam does not have traditional loops. Iteration is commonly written with
recursion or with functions from modules such as `gleam/list`.[^7]

```gleam
fn count(items: List(a)) -> Int {
  case items {
    [] -> 0
    [_, ..rest] -> 1 + count(rest)
  }
}
```

For long-running recursive functions, tail calls matter. Gleam supports tail call
optimisation, allowing a function call in tail position to reuse the current
stack frame.[^8]

The standard library often gives the clearer version:

```gleam
import gleam/list

pub fn double_all(numbers: List(Int)) -> List(Int) {
  list.map(numbers, fn(n) { n * 2 })
}
```

## Data shapes

Functional programs often model data with a small set of explicit shapes. Gleam
custom types make those shapes visible:

```gleam
pub type Login {
  Anonymous
  SignedIn(name: String)
}
```

Pattern matching then handles each shape:

```gleam
fn display(login: Login) -> String {
  case login {
    Anonymous -> "guest"
    SignedIn(name) -> name
  }
}
```

This style is valuable to a compiler because the possible cases are part of the
program structure. Name resolution, type checking, exhaustiveness checking, and
code generation can all use that structure.

[^1]: Gleam FAQ, "Does Gleam have mutable state?":
    https://gleam.run/frequently-asked-questions
[^2]: Gleam Language Tour, "Assignments":
    https://tour.gleam.run/basics/assignments/
[^3]: Gleam Language Tour, "Functions":
    https://tour.gleam.run/functions/functions/
[^4]: Gleam Language Tour, "Higher order functions":
    https://tour.gleam.run/functions/higher-order-functions/
[^5]: Gleam Language Tour, "Anonymous functions":
    https://tour.gleam.run/functions/anonymous-functions/
[^6]: Gleam Language Tour, "Pipelines":
    https://tour.gleam.run/functions/pipelines/
[^7]: Gleam Language Tour, "List recursion":
    https://tour.gleam.run/flow-control/list-recursion/
[^8]: Gleam Language Tour, "Tail calls":
    https://tour.gleam.run/flow-control/tail-calls/
