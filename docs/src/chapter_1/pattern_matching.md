# Gleam pattern matching

Pattern matching lets a program inspect the shape of a value.[^1]

```gleam
pub fn describe(number: Int) -> String {
  case number {
    0 -> "zero"
    1 -> "one"
    _ -> "many"
  }
}
```

The `_` pattern is a discard. It matches without binding a name.

## Variable and literal patterns

Patterns can bind names:

```gleam
case pair {
  #(name, age) -> name
}
```

The name is available only in the branch body. A variable pattern always matches
and assigns the matched value to the variable.[^2]

Literal patterns match exact values:

```gleam
case answer {
  42 -> "yes"
  _ -> "no"
}
```

String patterns can match a prefix with the string append operator:

```gleam
case message {
  "Hello, " <> name -> name
  _ -> "unknown"
}
```

## Custom types and records

Custom-type constructors are common in patterns:

```gleam
case direction {
  North -> "north"
  South -> "south"
  _ -> "somewhere else"
}
```

Records can be pattern matched to extract labelled fields.[^3]

```gleam
case user {
  User(name: name, age: _) -> name
}
```

The `..` syntax discards fields that are not needed:

```gleam
case user {
  User(name: name, ..) -> name
}
```

## Lists and tuples

Tuple patterns match fixed-size tuples:

```gleam
case point {
  #(0, y) -> y
  #(x, _) -> x
}
```

List patterns can match exact lengths, empty lists, or a head and tail.[^4]

```gleam
case numbers {
  [] -> "empty"
  [one] -> "one item"
  [first, ..rest] -> "many items"
}
```

The `..` list pattern matches the rest of the list. It can bind the rest to a
name or discard it.

## Multiple subjects, alternatives, aliases, and guards

A `case` expression can match multiple subjects at once.[^5]

```gleam
case x, y {
  0, 0 -> "both zero"
  0, _ -> "first zero"
  _, 0 -> "second zero"
  _, _ -> "neither zero"
}
```

Alternative patterns use `|` when several patterns should run the same branch.
If one alternative binds a variable, the alternatives for that branch must bind
the same variable with the same type.[^6]

```gleam
case status {
  200 | 201 | 204 -> "ok"
  _ -> "not ok"
}
```

The `as` operator gives a name to a matched sub-pattern.[^7]

```gleam
case lists {
  [[_, ..] as first, ..] -> first
  _ -> []
}
```

Guards add a boolean condition to a pattern.[^8]

```gleam
case score {
  n if n >= 90 -> "great"
  _ -> "keep going"
}
```

## Exhaustiveness

Gleam checks that `case` expressions cover all possible values.[^1] If a custom
type gains a new variant, pattern matches over that type may need to be updated.
Custom types and pattern matching work well together here: the compiler can help
keep code aligned with the data model.

For a compiler, patterns introduce names, check value shapes, depend on type
information, and eventually become branching code.

[^1]: Gleam Language Tour, "Case expressions":
    https://tour.gleam.run/flow-control/case-expressions/
[^2]: Gleam Language Tour, "Variable patterns":
    https://tour.gleam.run/flow-control/variable-patterns/
[^3]: Gleam Language Tour, "Record pattern matching":
    https://tour.gleam.run/data-types/record-pattern-matching/
[^4]: Gleam Language Tour, "List patterns":
    https://tour.gleam.run/flow-control/list-patterns/
[^5]: Gleam Language Tour, "Multiple subjects":
    https://tour.gleam.run/flow-control/multiple-subjects/
[^6]: Gleam Language Tour, "Alternative patterns":
    https://tour.gleam.run/flow-control/alternative-patterns/
[^7]: Gleam Language Tour, "Pattern aliases":
    https://tour.gleam.run/flow-control/pattern-aliases/
[^8]: Gleam Language Tour, "Guards":
    https://tour.gleam.run/flow-control/guards/
