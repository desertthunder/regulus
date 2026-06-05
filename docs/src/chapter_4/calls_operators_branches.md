# Calls, operators, and branches

Expression checking is where type information turns into diagnostics. The
checker does not only assign types; it compares actual types against the types
required by the language construct being checked.

## Function calls

A function type stores parameter types and a return type:

```text
fn(Int, String) -> Bool
```

A call checks three things:

1. The callee expression has a function type.
2. The argument count matches the parameter count.
3. Each argument type matches the corresponding parameter type.

```gleam
fn same(x: Int, y: Int) -> Bool {
  x == y
}

fn main() {
  same(1, 2)
}
```

Here `same` has type `fn(Int, Int) -> Bool`, so `same(1, 2)` has type `Bool`.
If the call is `same(1, "two")`, the second argument is rejected because the
parameter requires `Int`.

## Operators

Operators are checked like small built-in functions. Each operator has operand
requirements and a result type. Gleam uses separate numeric operators for ints
and floats rather than overloading one arithmetic operator family.[^everything]

| Operator group | Operand types       | Result type |
| -------------- | ------------------- | ----------- |
| `+`, `-`, `*`  | `Int`, `Int`        | `Int`       |
| `+.`, `-.`     | `Float`, `Float`    | `Float`     |
| `&&`, `||`     | `Bool`, `Bool`      | `Bool`      |
| `<>`           | `String`, `String`  | `String`    |
| `==`, `!=`     | same type on both sides | `Bool`  |

Equality is broader than arithmetic: `==` and `!=` can be used with values of
any type, but both sides must have the same type.[^equality]

This keeps error reporting direct. The checker does not need to guess whether
`+` means integer addition or string concatenation because Gleam has separate
operators for those operations.

## Blocks

A block's type is the type of its last expression. Gleam evaluates block
expressions in order and returns the value of the last expression.[^blocks]
Statements that only bind names have type `Nil` as intermediate steps in
Regulus, but they do not decide the block result unless they are the final
statement.

```gleam
{
  let name = "Ada"
  name
}
```

This block has type `String`.

Nested blocks push a new scope. Names defined inside the block are not visible
after the block ends.

## Case expressions

A `case` expression has subject expressions, patterns, optional guards, and
branch values. Gleam uses `case` for pattern matching, and the official tour
describes it as the language's main flow-control expression.[^case]

```gleam
case x {
  0 -> "zero"
  _ -> "many"
}
```

The checker first finds the subject type. Each pattern is then checked against
that type. Each branch value is checked, and all branch values must agree on one
result type. The example above has type `String`.

If one branch returns a different type, the `case` expression is rejected:

```gleam
case x {
  0 -> "zero"
  _ -> 1
}
```

Guards must have type `Bool`. A guarded branch does not prove exhaustiveness
because the guard can fail at runtime. Gleam also restricts guard expressions:
they cannot contain function calls, case expressions, or blocks.[^guards]

## Exhaustiveness and reachability

Type information also supports pattern coverage checks. Gleam performs
exhaustiveness checking for `case` expressions so patterns cover all possible
values of the matched data.[^case] Regulus currently checks simple
exhaustiveness for booleans, lists, tuples, and custom types. It also reports
unreachable branches after a previous unguarded pattern has covered the subject
type.

```gleam
case flag {
  _ -> 1
  True -> 2
}
```

The second branch cannot run because `_` already covers every `Bool`.

Coverage checking should remain a type-checking concern, not a lowering
concern. Lowering should receive a checked branch structure whose subject and
result types are already known.

[^everything]: Gleam Language Tour, "Everything", int and float operators: https://tour.gleam.run/everything/
[^equality]: Gleam Language Tour, "Equality": https://tour.gleam.run/basics/equality/
[^blocks]: Gleam Language Tour, "Blocks": https://tour.gleam.run/basics/blocks/
[^case]: Gleam Language Tour, "Case expressions": https://tour.gleam.run/flow-control/case-expressions/
[^guards]: Gleam Language Tour, "Guards": https://tour.gleam.run/flow-control/guards/
