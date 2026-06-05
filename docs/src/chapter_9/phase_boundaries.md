# Phase boundaries

The compiler's main architecture rule is that every phase has one owner and one
output contract. That keeps correctness local. When a bug appears in generated
Wasm, the compiler can ask which invariant was broken instead of treating the
whole pipeline as one large transformation.

This phase discipline is the practical version of a pass pipeline. LLVM divides
passes into analyses, transformations, and utilities: analyses compute
information, transformations mutate the program, and utilities do work that does
not fit either category.[^llvm-passes] MLIR makes the same distinction sharp by
treating analyses as cached, non-mutating computations and by letting passes
signal failure so the remaining pipeline does not run on invalid IR.[^mlir-pass]

## Parse

Parsing turns source text into a concrete syntax tree. Tree-sitter is designed
for concrete syntax trees that support editor use, incremental parsing, and
error recovery.[^tree-sitter]

Regulus uses that tree as input, but it does not let every later phase depend on
tree-sitter node names. The concrete tree is too close to parser details.

## AST build

The AST builder converts concrete syntax into compiler-owned Rust data:

```text
tree-sitter node -> ast::Expression
tree-sitter node -> ast::Declaration
tree-sitter node -> ast::Pattern
```

The AST preserves source spans, names, literal source text, annotations, and raw
syntax for constructs that are parsed but not fully supported yet. This is the
first representation later phases should depend on.

AST building is a transformation pass: it changes representation and discards
parser-only details. It must preserve the source details that later diagnostics
need, especially spans and source text for literals and annotations.

## Resolve

Name resolution turns textual references into known declarations. It handles
lexical scopes, imports, module-qualified references, namespaces, visibility,
constructors, fields, and pattern bindings.

Resolution output is a `ResolvedModule`: the AST plus a symbol table and
reference data. It does not decide whether calling a value is type-correct. It
only answers what the name refers to.

Resolution is mostly an analysis over the AST. It can report errors, and it can
produce symbol/reference tables, but it should not change expression meaning.
That separation keeps type checking from depending on textual lookup rules.

## Type check

Type checking consumes resolved syntax and produces `TypedModule`. It validates
calls, operators, records, constructors, patterns, branches, and annotations. It
also builds `ModuleInterface`, which carries function types, type declarations,
and constructor metadata for imports and later phases.

The typed output is the compiler's semantic boundary. Lowering should not need
to parse type annotation strings or infer local variable types.

Type checking is also where module-interface data becomes trustworthy. A later
module should import a checked function type, not an unverified annotation
string.

## Lower

Lowering turns typed syntax into core IR. This is where source-level constructs
become explicit runtime work:

- parameters and bindings become locals
- blocks become ordered instructions with a result
- calls become direct or indirect call nodes
- pattern matches become branch clauses and successful bindings
- assert patterns become explicit failure paths

Lowering still does not choose final machine instructions. It creates a smaller
compiler IR that the WebAssembly backend can emit deterministically.

Lowering is a transformation pass. Its output should make implicit source
semantics explicit: evaluation order, local allocation, pattern-failure paths,
and runtime-managed value operations. That makes later analyses simpler because
they inspect fewer constructs.

## Emit

WebAssembly emission turns core IR into WAT and then binary bytes. The core
specification defines validation and execution over modules, types,
instructions, and the text and binary formats.[^wasm-spec]

This phase owns target representation: scalar ABI values, managed-value
pointers, linear memory operations, imports, exports, runtime helpers, and
target diagnostics for shapes the backend cannot emit.

Emission should verify its own target invariants. LLVM encourages front ends to
verify generated IR before optimization because malformed IR can crash later
tools.[^llvm-passes] Regulus gets a similar guardrail from WAT assembly and
WebAssembly validation: bad stack types, invalid imports, or unsupported ABI
shapes should fail before an artifact is treated as executable.

[^tree-sitter]: Tree-sitter documentation, "Introduction": https://tree-sitter.github.io/tree-sitter/
[^wasm-spec]: WebAssembly Core Specification: https://webassembly.github.io/spec/core/
[^llvm-passes]: LLVM, "Analysis and Transform Passes": https://llvm.org/docs/Passes.html
[^mlir-pass]: MLIR, "Pass Management": https://mlir.llvm.org/docs/PassManagement/
