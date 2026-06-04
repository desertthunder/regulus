# Gleam functions, values, and expressions

Gleam programs are built from expressions. A function body is a block of
expressions, and the final expression is the return value.[^1]

```gleam
pub fn add_one(x: Int) -> Int {
  let one = 1
  x + one
}
```

There is no ordinary `return` statement. The function result is the value of the
last expression in the body.

## Values

Gleam has familiar scalar values:

```gleam
1
1.5
"hello"
True
False
Nil
```

`let` introduces a local binding:

```gleam
let name = "Lucy"
name
```

Values are immutable. Reusing a variable name creates a new binding rather than
changing the old value.[^2]

```gleam
let name = "Lucy"
let name = "Hello, " <> name
name
```

Blocks are expressions too. They have their own local scope, and the final
expression is the block value.[^3]

```gleam
let celsius = {
  let fahrenheit = 64
  { fahrenheit - 32 } * 5 / 9
}
```

## Calls and labels

Function calls use parentheses:

```gleam
add_one(41)
```

Arguments can have labels when a function defines them.[^4]

```gleam
pub fn replace(in string: String, each pattern: String, with replacement: String) {
  todo
}

replace(in: "one two", each: "one", with: "three")
```

Labels are checked at compile time. They do not allocate a dictionary or change
the runtime calling convention; they make call sites clearer and let labelled
arguments be passed in a different order.

## Operators and pipelines

Gleam has operators for arithmetic, comparison, boolean logic, equality, and
string concatenation. Numeric operators are not overloaded: integer and float
operations use different operators where the language needs that distinction.

Pipelines make nested calls read from top to bottom.[^5]

```gleam
import gleam/int
import gleam/string

pub fn label(count: Int) -> String {
  count
  |> int.to_string
  |> string.append(" items")
}
```

The pipe operator passes the value on its left into the call on its right. Gleam
libraries commonly put the subject of the operation first so functions compose
well in pipelines.

## Function values

Named functions and anonymous functions can be used as values.[^6]

```gleam
fn twice(value: Int, change: fn(Int) -> Int) -> Int {
  change(change(value))
}

pub fn main() {
  twice(1, fn(x) { x + 1 })
}
```

Function captures are a shorthand for a one-argument anonymous function that
immediately calls another function.[^7]

```gleam
import gleam/list
import gleam/string

pub fn shout(names: List(String)) -> List(String) {
  list.map(names, string.append(_, "!"))
}
```

This is equivalent to passing `fn(name) { string.append(name, "!") }`.

## Use, todo, panic, and assertions

`use` is syntax for calling a function that takes a callback as its final
argument.[^8] It avoids nested anonymous functions when working with
callback-style APIs.

`todo` and `panic` intentionally stop execution. They are useful while sketching
code or when a program reaches a state that should not happen.[^9] Assertions
let code state facts the compiler or runtime should check, such as whether a
pattern must match.

For a compiler, all of these constructs are expressions. Some produce ordinary
values, some introduce control flow, and some bind names that are visible in the
following expression body.

[^1]: Gleam Language Tour, "Functions":
    https://tour.gleam.run/functions/functions/
[^2]: Gleam Language Tour, "Assignments":
    https://tour.gleam.run/basics/assignments/
[^3]: Gleam Language Tour, "Blocks":
    https://tour.gleam.run/basics/blocks/
[^4]: Gleam Language Tour, "Labelled arguments":
    https://tour.gleam.run/functions/labelled-arguments/
[^5]: Gleam Language Tour, "Pipelines":
    https://tour.gleam.run/functions/pipelines/
[^6]: Gleam Language Tour, "Anonymous functions":
    https://tour.gleam.run/functions/anonymous-functions/
[^7]: Gleam Language Tour, "Function captures":
    https://tour.gleam.run/functions/function-captures/
[^8]: Gleam Language Tour, "Use":
    https://tour.gleam.run/advanced-features/use/
[^9]: Gleam Language Tour, "Todo":
    https://tour.gleam.run/advanced-features/todo/
