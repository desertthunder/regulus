# Pattern matching

Pattern matching is handled across parsing, resolution, type checking, lowering,
and WebAssembly emission. Each phase keeps the part it owns explicit so branch
behavior and diagnostics stay inspectable.

## Supported pattern forms

The compiler represents and checks:

- literal patterns
- discard patterns
- variable patterns
- tuple patterns
- list patterns
- constructor patterns
- nested patterns
- aliases
- multiple subjects
- guards
- `let assert` patterns
- bit-string patterns at the IR representation level

Record patterns are represented in syntax and metadata. Executable support can
expand as record lowering and backend behavior grow.

## Phase responsibilities

Parsing preserves the source shape and spans. Resolution binds names introduced
by patterns and rejects duplicate bindings in a single pattern. Type checking
checks each pattern against the subject type, validates branch result types, and
reports obvious unreachable or non-exhaustive branches.

Lowering turns source patterns into IR branch clauses with explicit tests and
bindings. Code generation then emits scalar comparisons, managed-value tag
tests, field loads for bound values, guard checks, fallthrough, and failure
paths.

## Diagnostics

Pattern diagnostics should point at the pattern or branch that caused the issue.
Common diagnostics include:

- wrong tuple arity
- list pattern used with a non-list subject
- unknown constructor or field
- constructor pattern with too many arguments
- duplicate binding name
- unreachable branch
- non-exhaustive case expression

## Backend behavior

A branch emits clauses in source order. Each clause emits tests for its
subjects, combines them with the guard when present, binds pattern values, and
evaluates the clause body. If no clause matches, code generation emits the
appropriate failure path.
