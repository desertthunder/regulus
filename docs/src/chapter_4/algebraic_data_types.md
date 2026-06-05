# Algebraic data types

Algebraic data types explain why custom types, tuples, records, lists, and
pattern matching belong in the type-checking chapter. They are not only runtime
shapes. They are type-level descriptions of the values a program may construct
and the cases a program must handle.

In ML-family languages, the usual building blocks are products and sums.
Products combine fields. Sums choose one constructor from a finite set. The
OCaml documentation presents tuples, records, and variants together as the basic
data forms that support pattern matching, and notes that variants and products
correspond to algebraic data types.[^ocaml-basic]

## Product types

A product type contains several pieces at the same time:

```gleam
#(String, Int)
```

A value of this type contains both a `String` and an `Int`. A record constructor
is also product-like:

```gleam
pub type Person {
  Person(name: String, age: Int)
}
```

The constructor has two fields. Type checking a construction of `Person` means
checking each supplied argument against the field type:

```gleam
Person(name: "Ada", age: 36)
```

The value has type `Person`. The fields are not independent values after the
constructor call; together they form one product payload inside one custom-type
value.

## Sum types

A sum type chooses between constructors:

```gleam
pub type Login {
  Anonymous
  SignedIn(name: String)
}
```

A `Login` value is either `Anonymous` or `SignedIn(String)`. Real World OCaml
uses the term algebraic data types for variants, records, and tuples, with
variants providing the choice between cases.[^rwo-variants]

This is the type-checking reason pattern matching can be exhaustive. If the
checker knows all constructors of `Login`, it can know that a `case` expression
has handled every shape:

```gleam
case login {
  Anonymous -> "guest"
  SignedIn(name) -> name
}
```

If a constructor is missing, the program may have a runtime path with no branch.
That is a static error or warning in ML-style languages, and Regulus treats the
same information as a type-checking responsibility.

## Constructors as typed functions

A constructor can be viewed as a function from field types to the custom type:

```text
Anonymous : Login
SignedIn : fn(String) -> Login
```

For generic types, constructors have generic function-like types:

```gleam
pub type Option(value) {
  Some(value)
  None
}
```

```text
Some : fn(value) -> Option(value)
None : Option(value)
```

When checking `Some(1)`, the checker substitutes `value = Int`, so the result is
`Option(Int)`. When matching on an `Option(String)`, the same substitution says
that `Some(name)` binds `name: String`.

## Nominal identity

Regulus treats custom types nominally. Two custom types with the same field
layout are not the same type:

```gleam
pub type UserId {
  UserId(Int)
}

pub type OrderId {
  OrderId(Int)
}
```

Both have one `Int` field, but `UserId` is not `OrderId`. This is why interface
metadata records type names and constructor names, not just field layouts. The
package-interface type model also represents named types with a package, module,
name, and parameters.[^package-type]

## What the checker must preserve

For each custom type, type checking needs:

- the type name and generic parameters
- whether the type is opaque
- each constructor name
- labelled and positional field types
- the constructor return type

Lowering can then decide representation. The type checker decides whether the
source program used the ADT correctly.

[^ocaml-basic]: OCaml Documentation, "Basic Data Types and Pattern Matching": https://ocaml.org/docs/basic-data-types
[^rwo-variants]: Real World OCaml, "Variants": https://dev.realworldocaml.org/variants.html
[^package-type]: `gleam_package_interface`, `Type`: https://gleam-package-interface.hexdocs.pm/gleam/package_interface.html#Type
