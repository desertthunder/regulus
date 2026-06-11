# Pattern matching in the compiler

Pattern matching is where several compiler phases meet. Parsing records the
source pattern. Name resolution decides which constructor or field each name
refers to. Type checking verifies that the pattern can match the subject type
and records the variables it binds. Lowering turns that checked pattern into IR
tests and explicit local writes. Code generation emits the tests, jumps, and
field loads needed to run the match.

Gleam uses `case` expressions for pattern matching and performs exhaustiveness
checking so the branches cover the possible subject values.[^gleam-case]
Regulus follows the same direction: pattern matching should be checked before
lowering, and lowering should receive enough type information to make bindings
and failure paths explicit.

This chapter covers:

- compiling pattern matching into tests and branches
- binding variables from nested patterns
- exhaustiveness, unreachable branches, and redundant patterns

## A compiler-shaped example

```gleam
pub type Result(ok, error) {
  Ok(ok)
  Error(error)
}

fn unwrap_or(result: Result(Int, String), default: Int) -> Int {
  case result {
    Ok(value) -> value
    Error(_) -> default
  }
}
```

The checker sees `result: Result(Int, String)`. That lets it instantiate
`Ok(ok)` as `Ok(Int)` and bind `value: Int`. Lowering then records that the
first branch succeeds only if the subject has the `Ok` constructor tag. If it
succeeds, the payload field is written into the local for `value`.

The backend should not rediscover any of that from source strings. It should
receive an IR branch with typed subjects, lowered patterns, branch bodies, and a
fallthrough failure path.

[^gleam-case]: Gleam Language Tour, "Case expressions": https://tour.gleam.run/flow-control/case-expressions/
