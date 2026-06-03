# Gleam custom types, records, tuples, and lists

Gleam uses custom types to model data with named variants.[^1]

```gleam
pub type Direction {
  North
  South
  East
  West
}
```

Variants can carry data:

```gleam
pub type User {
  User(name: String, age: Int)
}
```

This variant has fields named `name` and `age`. Values are built by calling the
constructor:

```gleam
let user = User(name: "Ada", age: 36)
```

Tuples group a fixed number of values:

```gleam
#("Ada", 42)
```

Lists hold zero or more values of the same type:

```gleam
[1, 2, 3]
```

Lists are linked lists in Gleam. They are a good fit for recursive algorithms and
pattern matching. Tuples are useful when a small fixed group of values belongs
together but does not need a named custom type.

Records and custom types are often the clearest way to model application data.
They make the possible shapes of a value explicit in the type definition.

[^1]: Gleam language tour: https://github.com/gleam-lang/language-tour
