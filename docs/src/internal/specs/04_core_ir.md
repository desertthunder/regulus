# Core IR

Core IR is the compiler's small, typed intermediate representation. It should be
closer to WebAssembly than Gleam, but still independent of a specific textual or
binary WASM emitter.

## Responsibilities

- Represent typed functions, locals, expressions, calls, and control flow.
- Make evaluation order explicit.
- Remove Gleam-specific surface syntax.
- Carry enough type and span metadata for backend errors and debug output.

## Initial IR shape

The first IR can be expression-oriented:

- Module
- Function
- Block
- Local get/set
- Literal constants
- Direct calls
- If / branch
- Return

A later pass can introduce lower-level instructions if stack-machine codegen
becomes easier from a flattened form.

## Lowering rules

- Top-level Gleam functions become core IR functions.
- Function parameters become core IR locals.
- `let` bindings become locals with explicit initialization.
- Blocks evaluate statements in order and yield the final expression.
- Runtime-managed values, such as strings, use an explicit representation type.

## Invariants

- All core IR values are typed.
- All variable references point to known locals or functions.
- No Gleam name-resolution rules remain.
- Unsupported typed AST constructs fail during lowering with a clear diagnostic.
