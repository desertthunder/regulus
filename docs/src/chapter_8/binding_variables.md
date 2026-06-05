# Binding variables from patterns

Pattern matching does not only choose a branch. It also introduces names. Those
names must have stable types and stable runtime locations before lowering and
code generation can use them.

## Simple names and aliases

A name pattern binds the whole subject:

```gleam
case value {
  x -> x
}
```

If `value` has type `Int`, then `x` has type `Int`.

An alias binds both the inner pattern and the whole matched value:

```gleam
case pair {
  #(left, right) as whole -> whole
}
```

Here `left` and `right` are bound from tuple fields, while `whole` is bound to
the full tuple. Lowering must preserve both facts. The alias local cannot be
reconstructed from just one field.

## Tuple and record fields

Tuple patterns bind by position:

```gleam
case point {
  #(x, y) -> x + y
}
```

If `point` has type `#(Int, Int)`, both `x` and `y` are `Int`. Lowering records
field paths such as subject 0, field 0 and subject 0, field 1.

Record and custom-type patterns bind through constructor metadata:

```gleam
case user {
  User(name:, age: years) -> years
}
```

The field label `name:` binds a local named `name` when no nested pattern is
written. The field `age: years` binds `years` from the `age` field. The checker
gets both field types from the constructor declaration.

## Lists

A list pattern binds elements and optionally a tail:

```gleam
case items {
  [head, ..tail] -> head
  [] -> 0
}
```

If `items` has type `List(Int)`, then `head: Int` and `tail: List(Int)`.
Lowering needs a binding path for the head field and a binding path for the tail
list. The backend can then load those fields from the runtime list
representation after the list-cons test succeeds.

## Bit strings

Bit-string patterns introduce names from bit segments:

```gleam
case bits {
  <<tag:8, rest:bits>> -> tag
}
```

Regulus currently treats names extracted from bit-string pattern segments as
integers in the type checker. Lowering keeps the segment structure in
`IrPattern::BitString`, and the backend tests the subject as a bit-array value.

That is only a starting point. A complete implementation needs segment sizes,
unit options, signedness, endianness, binary tails, and failure diagnostics that
point to the segment that cannot match.

## Binding after success

Bindings should be installed only after the pattern succeeds. If the compiler
binds a field before checking a later nested literal, a failed branch can leave
locals with values that do not correspond to a successful match.

Regulus models this with successful bindings on branch clauses. The type checker
decides the type of each binding. Lowering records where the value comes from.
The backend writes the locals after the clause tests pass.
