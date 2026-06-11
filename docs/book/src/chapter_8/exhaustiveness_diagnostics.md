# Exhaustiveness and redundant patterns

Pattern matching gives the compiler enough structure to report errors that an
ordinary `if` chain cannot. The two important diagnostics are:

- non-exhaustive match: some possible value has no branch
- redundant branch: a branch can never run

Maranget describes these as non-exhaustive matches and useless clauses, the two
basic anomalies of ML pattern matching.[^maranget-warnings]

## Exhaustiveness

For a boolean, exhaustiveness is direct:

```gleam
case flag {
  True -> 1
}
```

This misses `False`.

For a custom type, exhaustiveness depends on the constructor set:

```gleam
pub type Direction {
  North
  South
  East
  West
}
```

A `case Direction` expression must cover all four constructors or include a
catch-all pattern:

```gleam
case direction {
  North -> 0
  South -> 1
  East -> 2
  West -> 3
}
```

The checker cannot perform this analysis unless it knows the subject type and
the constructors belonging to that type.

## Redundancy and unreachable branches

A branch is redundant when earlier unguarded patterns already cover all values
it could match:

```gleam
case direction {
  _ -> 0
  North -> 1
}
```

`North` is unreachable because `_` matched every direction first.

Nested redundancy is more subtle:

```gleam
case result {
  Ok(_) -> 1
  Ok(0) -> 2
  Error(_) -> 3
}
```

The second branch is unreachable because `Ok(_)` already covered every `Ok`
value. A complete checker needs recursive coverage reasoning over constructors
and product fields, not only top-level constructor names.

## Guards

Guarded branches do not prove coverage:

```gleam
case number {
  x if x > 0 -> "positive"
}
```

The pattern `x` covers every number, but the guard can fail. For coverage, the
branch cannot be treated as a catch-all. Gleam guards must evaluate to `True`
for the pattern to match, and the language restricts guard expressions so they
cannot contain function calls, case expressions, or blocks.[^guards]

## Current Regulus behavior

Regulus currently checks:

- missing `True` or `False` for boolean subjects
- simple list coverage for empty and non-empty lists
- tuple catch-all coverage
- missing constructors for custom types
- unreachable branches after an unguarded covering pattern

That is useful but incomplete. The next level is a usefulness algorithm over a
pattern matrix: each new clause is checked for whether it matches any value not
matched by previous clauses, and the whole matrix is checked for whether it
covers every value of the subject type.

## Diagnostic quality

The best diagnostic should name the missing shape:

```text
case expression is not exhaustive; missing West
```

For nested patterns, it should show a witness pattern when possible:

```text
missing Ok(_)
```

For redundancy, it should point at the unreachable branch and explain the prior
coverage:

```text
this branch is unreachable because a previous `_` branch covers all values
```

Those messages require type information, constructor metadata, and spans from
the original patterns.

[^maranget-warnings]: Luc Maranget, "Warnings for pattern matching": https://www.cambridge.org/core/journals/journal-of-functional-programming/article/warnings-for-pattern-matching/3165B75113781E2431E3856972940347
[^guards]: Gleam Language Tour, "Guards": https://tour.gleam.run/flow-control/guards/
