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

Patterns can bind names:

```gleam
case pair {
  #(name, age) -> name
}
```

They can match custom-type constructors:

```gleam
case direction {
  North -> "north"
  South -> "south"
  _ -> "somewhere else"
}
```

They can also match lists:

```gleam
case numbers {
  [] -> 0
  [first, ..rest] -> first
}
```

Pattern matching is one of the main ways Gleam code works with custom types,
tuples, and lists. It lets each branch describe the shape of data it accepts,
and each branch returns a value like any other expression.

[^1]: Gleam language tour: https://github.com/gleam-lang/language-tour
