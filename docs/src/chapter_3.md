# Type systems

A type system classifies values and expressions. In Gleam, `1` is an `Int`,
`"hello"` is a `String`, `True` is a `Bool`, and a function has a function type
that describes its parameters and return value.

Types let the compiler reject programs before they run:

```gleam
fn id(x: Int) -> Int {
  x
}

fn main() {
  id("not an int")
}
```

The call to `id` is shaped correctly: it has one argument. Name resolution can
also find the function named `id`. The problem is that the argument has the wrong
type. `id` expects an `Int`, but the call passes a `String`.

## Checking expressions

A type checker walks expressions and assigns each one a type. Simple literals
are direct:

```text
1       : Int
1.5     : Float
"hi"    : String
True    : Bool
Nil     : Nil
```

Variables get their types from bindings. In this function, the parameter
annotation says that `name` is a `String`, so the expression `name` also has type
`String`:

```gleam
fn echo(name: String) -> String {
  name
}
```

A `let` binding gets its type from the value on the right-hand side:

```gleam
fn main() {
  let x = 1
  x
}
```

Here `1` has type `Int`, so `x` has type `Int`, and the final expression has type
`Int` too.

## Function types

A function type records the parameter types and the return type. The function
below has type `fn(Int) -> Int`:

```gleam
fn id(x: Int) -> Int {
  x
}
```

A call checks two things:

1. The value being called is a function.
2. The argument types match the function's parameter types.

So this call is valid:

```gleam
id(1)
```

This call has the wrong arity:

```gleam
id(1, 2)
```

And this call has the wrong argument type:

```gleam
id("one")
```

Stephen Diehl's Typechecker Zoo shows this same core idea in several type
systems: expressions are checked against rules, and those rules produce or
compare types.[^1]

## Inference and annotations

Some languages require many type annotations. Some infer most types. Hindley-
Milner is a famous family of inference algorithms used by languages in the ML
tradition.[^2] It can infer types for many programs without requiring the
programmer to write them down.

A tiny example of inference is the `let` binding above. The programmer does not
write `x: Int`, but the compiler can infer it from `1`.

This compiler currently uses a smaller approach. Function parameters need type
annotations, and local bindings can often be inferred from their values:

```gleam
fn add_one(x: Int) -> Int {
  let one = 1
  x
}
```

The annotation on `x` gives the checker a type for the parameter. The literal
`1` gives the checker a type for `one`.

A fuller inference system would introduce type variables, collect constraints,
and solve those constraints. Hindley-Milner tutorials often present this as a
process of generating equations such as "the argument type must equal the
parameter type" and then unifying them.[^3]

## Branches and pattern matching

A `case` expression has subjects, patterns, and branch values:

```gleam
fn choose(x: Int) {
  case x {
    0 -> 1
    _ -> 2
  }
}
```

The patterns are checked against the subject type. Since `x` is an `Int`, the
literal pattern `0` is valid. The discard pattern `_` accepts the subject without
introducing a name.

The branch values also need to agree. This `case` has type `Int` because both
branches return integers:

```gleam
case x {
  0 -> 1
  _ -> 2
}
```

This one is rejected because one branch returns an `Int` and the other returns a
`String`:

```gleam
case x {
  0 -> 1
  _ -> "two"
}
```

Pattern matching can become much richer than this. Real compilers often lower
patterns into decision trees or related forms so matching can be checked and
compiled efficiently.[^4] The current checker only needs the simpler question:
does this pattern make sense for the subject type, and do all branches produce a
compatible result?

## What this compiler checks today

The current type checker handles:

- `Int`, `Float`, `String`, `Bool`, and `Nil`
- function types
- typed function parameters
- optional function return annotations
- literals
- variables
- local `let` bindings
- direct function calls
- arity checks
- argument type checks
- simple `case` expressions
- branch type compatibility

It reports type errors with source spans, so errors point back to the expression
or annotation that caused the problem.

[^1]: Stephen Diehl, "Typechecker Zoo": https://www.stephendiehl.com/posts/typechecker_zoo/
[^2]: Stephen Diehl, "Hindley-Milner Inference": https://smunix.github.io/dev.stephendiehl.com/fun/006_hindley_milner.html
[^3]: Himanshu Stimsina, "Implementing a Hindley-Milner Type System, Part 1": https://blog.stimsina.com/post/implementing-a-hindley-milner-type-system-part-1
[^4]: Compiler Club, "Compiling Pattern Matching": https://compiler.club/compiling-pattern-matching/
