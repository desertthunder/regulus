# Unification and substitution

Regulus doesn't implement full Hindley-Milner inference, but it already
uses one of the central ideas: a generic type variable can be replaced by a
concrete type when a use site provides enough information.

That operation is substitution. A full ML-style inference engine gets those
substitutions by unification: solving type equations such as `List(a) =
List(Int)` by discovering `a = Int`. Cornell's type-checking notes describe the
typing context as a map from identifiers to types, while Hindley-Milner[^hm]
presentations add constraints and unification to infer missing types from that
context.[^cornell-typecheck]

## Type equations

Checking a call produces equations:

```gleam
fn first(items: List(item)) -> item {
  todo
}

first([1, 2, 3])
```

The parameter type is `List(item)`. The argument type is `List(Int)`. The
checker needs these to agree:

```text
List(item) = List(Int)
```

The outer constructors match, so the element types must match:

```text
item = Int
```

Substituting `Int` for `item` gives the return type `Int`.

## Constructor substitution

The same mechanism applies to custom constructors:

```gleam
pub type Result(ok, error) {
  Ok(ok)
  Error(error)
}
```

If a subject has type `Result(Int, String)`, matching `Ok(value)` should bind
`value: Int`. The constructor's declared return type is:

```text
Result(ok, error)
```

The concrete subject type is:

```text
Result(Int, String)
```

Matching the two records the substitutions:

```text
ok = Int
error = String
```

Those substitutions are then applied to constructor fields.

## Occurs checks and principality

A fuller unifier needs an occurs check so a type variable cannot be unified with
a type that contains itself, such as `a = List(a)`. Hindley-Milner systems also
try to infer a principal type: the most general type that still describes the
expression. Those properties matter once the compiler infers function
signatures, polymorphic locals, and higher-order values.

Regulus avoids most of that complexity today by requiring parameter annotations
and using direct equality plus limited generic substitution. That is simpler,
but it also means it must not claim to support full ML inference.

## Where this should grow

The next step is not to scatter ad hoc substitutions through the checker. It is
to make constraints explicit:

- expression `e` has inferred type `t`
- annotation `A` requires `t = A`
- call argument type must equal parameter type
- branch result types must equal one result type
- constructor subject type must equal constructor return type

Once constraints are explicit, unification can solve them and diagnostics can
point to the specific equation that failed.

[^cornell-typecheck]: Cornell CS3110, "Type Checking": https://cs3110.github.io/textbook/chapters/interp/typecheck.html

[^hm]: Stephen Diehl, "Hindley-Milner Inference": https://smunix.github.io/dev.stephendiehl.com/fun/006_hindley_milner.html
