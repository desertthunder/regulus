# Pattern matching

Pattern matching affects parsing, type checking, exhaustiveness diagnostics,
lowering, and code generation. Full support needs a dedicated design rather than
ad-hoc handling in `case` expressions.

## Pattern forms

- literals
- discard patterns
- variable patterns
- tuple patterns
- list patterns
- record patterns
- constructor patterns
- nested patterns
- multiple subjects
- guards
- `let assert` patterns

## Responsibilities

- Bind names introduced by patterns.
- Type-check patterns against subject types.
- Check branch result compatibility.
- Report unreachable or redundant branches where possible.
- Report non-exhaustive matches where possible.
- Lower patterns into a decision tree or another explicit matching IR.

## Output

Lowering should produce branch logic that no longer depends on Gleam pattern
syntax. Code generation should receive tests, bindings, and branch targets in an
explicit form.
