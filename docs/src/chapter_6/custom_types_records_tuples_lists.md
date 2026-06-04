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

Both the type name and constructor names start with uppercase letters. A value of
`Direction` is one of the listed variants.

## Records and constructors

Variants can carry data. In Gleam, a variant that carries fields is called a
record.[^2]

```gleam
pub type User {
  User(name: String, age: Int)
}
```

Values are built by calling the constructor:

```gleam
let ada = User(name: "Ada", age: 36)
let joe = User("Joe", 41)
```

Field labels can be used when constructing records. Labels are clearer for
records with more than one field because they make the meaning of each argument
visible at the call site.

Fields can be accessed with dot syntax:

```gleam
ada.name
```

Record updates create a new value from an existing record with selected fields
changed.[^3]

```gleam
let older = User(..ada, age: 37)
```

The original `ada` value is unchanged. Gleam data is immutable, so update syntax
builds another value rather than mutating the old one.

## Multiple variants

Custom types can model choices where each variant has different data:

```gleam
pub type Login {
  Anonymous
  SignedIn(name: String)
  Failed(reason: String)
}
```

Pattern matching is how code handles those shapes:

```gleam
fn label(login: Login) -> String {
  case login {
    Anonymous -> "guest"
    SignedIn(name) -> name
    Failed(reason) -> "failed: " <> reason
  }
}
```

## Opaque types

An opaque type can be public while its constructors remain private to the module
that defines it.[^4]

```gleam
pub opaque type PositiveInt {
  PositiveInt(inner: Int)
}
```

Other modules can use `PositiveInt` in type annotations, but they cannot build or
pattern match on `PositiveInt(inner: ...)` directly. This lets a module expose a
safe API, such as a smart constructor, while hiding representation details.

## Tuples

Tuples group a fixed number of values:

```gleam
#("Ada", 42)
```

They are useful for small groups of values, especially return values with two or
three parts.[^5]

```gleam
fn bounds() -> #(Int, Int) {
  #(0, 10)
}
```

Tuple elements can be accessed by position:

```gleam
let pair = #("Ada", 42)
pair.0
pair.1
```

If a tuple starts to carry domain meaning, a custom type is clearer.

## Lists

Lists hold zero or more values of the same type:

```gleam
[1, 2, 3]
["a", "b", "c"]
```

`List` is generic, so `[1, 2, 3]` has type `List(Int)` and `["a", "b"]` has type
`List(String)`.[^6] Lists are immutable singly linked lists. Adding or removing
from the front is efficient, while indexing into the middle is not the usual way
to work with them.

The spread syntax can prepend values to an existing list:

```gleam
let numbers = [2, 3]
let more = [0, 1, ..numbers]
```

The same shape appears in list patterns:

```gleam
case numbers {
  [] -> 0
  [first, ..rest] -> first
}
```

The `gleam/list` module provides common list operations such as `map`, `filter`,
`fold`, `first`, and `reverse`.[^7]

```gleam
import gleam/list

pub fn double_all(numbers: List(Int)) -> List(Int) {
  list.map(numbers, fn(n) { n * 2 })
}
```

[^1]: Gleam Language Tour, "Custom types":
    https://tour.gleam.run/data-types/custom-types/
[^2]: Gleam Language Tour, "Records":
    https://tour.gleam.run/data-types/records/
[^3]: Gleam Language Tour, "Record updates":
    https://tour.gleam.run/data-types/record-updates/
[^4]: Gleam Language Tour, "Opaque types":
    https://tour.gleam.run/advanced-features/opaque-types/
[^5]: Gleam Language Tour, "Tuples":
    https://tour.gleam.run/data-types/tuples/
[^6]: Gleam Language Tour, "Lists":
    https://tour.gleam.run/basics/lists/
[^7]: `gleam/list` standard library docs:
    https://hexdocs.pm/gleam_stdlib/gleam/list.html
