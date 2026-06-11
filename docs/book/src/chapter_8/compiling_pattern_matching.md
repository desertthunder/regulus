# Compiling pattern matching

A source `case` expression is a list of patterns and branch bodies. Executable
code needs tests, jumps, field extraction, and a fallback when no branch
matches.

```gleam
case status {
  Draft -> 0
  Published -> 1
  Archived -> 2
}
```

For a custom type, the first useful test is usually the constructor tag:

```text
if tag(status) == Draft     -> 0
if tag(status) == Published -> 1
if tag(status) == Archived  -> 2
fallthrough                 -> panic
```

For nested patterns, successful tests are followed by field extraction:

```gleam
case user {
  User(name: "Ada", age:) -> age
  _ -> 0
}
```

The compiler needs to test the outer constructor, load the `name` field, compare
it with `"Ada"`, and only then bind `age`.

## Decision trees

ML-family compilers often compile pattern matching into decision trees. A
decision tree chooses one subject occurrence to test, branches on the result,
and continues until it reaches a successful action or failure. Maranget's work
on compiling ML pattern matching studies decision trees and notes their key
advantage: a given subject subterm does not need to be tested more than once in
the tree.[^maranget-trees]

The common matrix presentation starts with rows of clauses and columns of
subject occurrences. Colin James' explanation of Maranget's algorithm describes
the pattern matrix, occurrence vector, and action vector as the central data
structures.[^compiler-club]

Regulus does not need the full matrix algorithm immediately, but the same
separation is useful:

- test selection: what part of the subject is inspected next
- success action: which branch body runs
- binding action: which fields are copied into locals
- failure action: what happens when no branch matches

## Current Regulus lowering

Regulus currently lowers a `case` expression into an IR `Branch`:

```text
Branch {
  subjects,
  clauses,
  fallthrough
}
```

Each clause stores lowered patterns, an optional guard, successful bindings, and
the lowered branch body. That is still higher-level than a decision tree, but it
is much closer to code generation than the source AST.

For a `let assert` or non-name `let` pattern, lowering emits `AssertMatch`.
That instruction has a value, a lowered pattern, and a failure path. The backend
can turn a failed assertion into a trap or panic path.

## Backend shape

The WebAssembly backend currently emits direct tests for each clause:

1. Test each subject against the clause pattern.
2. Test the guard if one exists.
3. Bind values for the successful pattern.
4. Emit the branch body.
5. Try the next clause if the test failed.

This is simple and readable. It is not always optimal, because the same subject
field may be tested more than once across clauses. A later decision-tree pass
can optimize that without changing the type-checking contract.

[^maranget-trees]: Luc Maranget, "Compiling Pattern Matching to Good Decision Trees": https://moscova.inria.fr/~maranget/papers/ml05e-maranget.pdf
[^compiler-club]: Colin James, "Compiling Pattern Matching": https://compiler.club/compiling-pattern-matching/
