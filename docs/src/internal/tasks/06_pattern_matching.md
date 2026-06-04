# Pattern matching tasks

## Goal

Support full Gleam pattern matching through type checking, lowering, and code
generation.

## Completed current subset

- [x] Bind names introduced by variable patterns in `case` branches.
- [x] Preserve wildcard/discard patterns through lowering.
- [x] Parse scalar literal patterns, including `True`, `False`, and `Nil`.
- [x] Type-check scalar literal, variable, and discard patterns.
- [x] Type-check multiple-subject `case` pattern counts.
- [x] Lower supported `case` patterns into explicit IR branch clauses.
- [x] Represent pattern bindings in IR so branch bodies can read captured
      values.
- [x] Emit WebAssembly control flow for scalar `case` expressions.
- [x] Emit WebAssembly local bindings for successful variable patterns.
- [x] Add Wasmtime tests for scalar branch fallthrough, bool patterns, and
      captured pattern values.

## Remaining full Gleam support

### AST and parsing

- [x] Replace raw executable pattern handling with explicit AST pattern nodes for
      tuple, list, record, constructor, nested, spread/rest, and aliased patterns
      where Gleam allows them.
- [x] Represent guards on case clauses with source spans.
- [x] Represent `let assert` as a pattern-bearing binding form, including its
      failure semantics.
- [x] Preserve source spans for each pattern node, constructor name, field name,
      and bound variable.
- [x] Add AST fixtures/snapshots for nested, constructor, record, list, tuple,
      guard, multi-subject, and `let assert` patterns.

### Name resolution

- [x] Bind names introduced by tuple, list, record, constructor, nested, and
      `let assert` patterns.
- [x] Resolve constructor names in patterns separately from variable bindings.
- [x] Resolve record fields used in record patterns.
- [x] Resolve qualified and imported constructors in patterns, including public
      and private visibility checks.
- [x] Reject duplicate variable bindings in one pattern where Gleam disallows
      them.
- [x] Ensure branch-local bindings are visible to the guard and branch body, but
      not outside the branch.
- [x] Add resolver fixtures for shadowing, imported constructors, branch-local
      names, and invalid duplicate bindings.

### Type checking and diagnostics

- [ ] Type-check tuple patterns by arity and element type.
- [ ] Type-check list patterns by element type, including empty lists and
      spread/rest patterns where supported.
- [ ] Type-check record patterns against record field metadata.
- [ ] Type-check constructor patterns against constructor parameter metadata,
      including imported constructors and generic custom types.
- [ ] Type-check nested patterns recursively while preserving useful diagnostic
      spans.
- [ ] Type-check guards as `Bool` expressions.
- [ ] Type-check `let assert` patterns and report impossible static matches
      where possible.
- [ ] Continue checking branch result compatibility after pattern checking.
- [ ] Add exhaustiveness diagnostics for booleans, tuples, lists, and custom-type
      constructors where the compiler can prove missing cases.
- [ ] Add redundancy diagnostics for unreachable branches where the compiler can
      prove a previous branch already covers the same values.
- [ ] Treat guards conservatively so guarded branches do not incorrectly make a
      match exhaustive.
- [ ] Add diagnostic snapshots for invalid patterns, non-exhaustive matches, and
      redundant branches.

### Lowering and WebAssembly

- [ ] Lower tuple, list, record, constructor, nested, guard, multi-subject, and
      `let assert` patterns into explicit matching IR.
- [ ] Ensure lowered matching logic no longer depends on Gleam pattern syntax.
- [ ] Emit runtime-backed tests for tuple, list, record, and custom-type
      constructor patterns.
- [ ] Emit guard checks after structural pattern tests and before branch bodies.
- [ ] Emit fallback code for non-matching branches and `let assert` failures.
- [ ] Add Wasmtime tests for nested matches, constructor matches, guards,
      multi-subject matches, and assertion failures.

## Done when

Supported patterns compile to explicit matching logic, and invalid patterns
produce source-spanned diagnostics.
