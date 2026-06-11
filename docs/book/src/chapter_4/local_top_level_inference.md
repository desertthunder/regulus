# Local and top-level inference

Type inference is not one uniform operation. A compiler uses different amounts
of inference at different boundaries. Local expression inference can be
aggressive because the whole expression is in front of the checker. Top-level
and module-boundary inference must preserve stable signatures for later modules.

## Local inference

A local binding can get its type from the value assigned to it:

```gleam
fn main() {
  let count = 1
  count
}
```

The literal `1` has type `Int`, so `count` is added to the current scope as
`Int`. No annotation is needed for the binding.

The same rule works for compound values:

```gleam
let pair = #(1, "name")
let flags = [True, False]
```

`pair` has type `#(Int, String)`. `flags` has type `List(Bool)`, because every
list element must agree on the same element type.

Regulus implements local inference by checking the right-hand expression first
and then binding the pattern to that type. For a variable pattern, the name gets
the whole value type. For a tuple, list, or constructor pattern, nested names
receive the type of the field they match.

## Function parameters

Function parameters are different. Regulus requires parameter annotations:

```gleam
fn echo(name: String) -> String {
  name
}
```

This keeps function checking straightforward. Before checking the body, the
checker pushes a new scope and inserts each annotated parameter. If an
annotation is missing, the checker reports a type error instead of inventing an
unconstrained type variable.

The official Gleam language supports type annotations for values and functions,
and uses them to state the type the compiler should check.[^annotations]
Regulus leans on that model while it grows toward fuller inference.

## Function returns

A function return annotation is optional in Regulus:

```gleam
fn answer() {
  42
}
```

The body has type `Int`, so the function type is `fn() -> Int`. If the return
annotation is present, the body must match it:

```gleam
fn answer() -> Int {
  42
}
```

This means top-level function collection has two phases:

1. Collect functions whose full signatures are available from annotations.
2. Check function bodies and record inferred return types.

The first phase is important for calls between functions. A call can only be
checked when the callee's parameter and return types are already known.

## Generics

Gleam generic functions use lowercase type variables. A variable stands for a
specific concrete type at each use, not for an untyped `any` value.[^generics]

```gleam
fn identity(value: value) -> value {
  value
}
```

In a full inference engine, each use of a generic function instantiates fresh
type variables and then unifies them with the argument types. Hindley-Milner
systems are known for inferring many such types using constraints,
generalization at `let`, and unification.[^hm]

Regulus has a smaller substitution model today. A constructor such as:

```gleam
pub type Box(value) {
  Box(value)
}
```

has a constructor return type `Box(value)`. When matching or updating a concrete
`Box(Int)`, the checker substitutes `value = Int` into the constructor fields.
That is enough for generic custom-type fields without claiming to implement full
Hindley-Milner inference.

## Practical invariant

The useful invariant is simple:

- top-level signatures provide stable entry points
- local expressions infer from checked values
- generic variables are substituted only when there is a concrete use site
- unresolved or unsupported annotations produce diagnostics before lowering

[^annotations]: Gleam Language Tour, "Type annotations": https://tour.gleam.run/basics/type-annotations/
[^generics]: Gleam Language Tour, "Generic functions": https://tour.gleam.run/functions/generic-functions/
[^hm]: Stephen Diehl, "Hindley-Milner Inference": https://smunix.github.io/dev.stephendiehl.com/fun/006_hindley_milner.html
