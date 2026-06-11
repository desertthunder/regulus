# The current resolver

This compiler's resolver builds a small symbol table from the compiler-owned
AST. It records symbols in separate namespaces and records references as either
direct symbol references or qualified module-member references.

The current resolver handles:

- top-level function names
- imported module names and aliases
- unqualified value imports
- unqualified type and constructor imports
- prelude type names
- function parameters
- local `let` and `let assert` bindings
- variable references
- nested block shadowing
- simple `case` clause pattern bindings
- custom type names
- constructor names
- record field names
- qualified module references such as `io.println`
- qualified constructor patterns across project modules
- public/private checks for project module members

It reports:

- duplicate names in the same scope and namespace
- duplicate bindings within one pattern
- unknown names at their source location
- ambiguous unqualified imports
- access to private members from another module

## Resolver shape

The resolver performs three collection steps for each module:

```text
collect prelude names
collect imports
collect top-level declarations
```

After collection, it walks function bodies and resolves expressions and
patterns. This order allows a function to call another function that appears
later in the file because all top-level declarations are known before bodies are
checked.

## Namespaces

The resolver stores each symbol under a `(namespace, name)` key. This prevents a
type name, value name, constructor name, field name, and module name from being
accidentally treated as the same declaration.

```text
(Type, "Result")        -> prelude type
(Constructor, "Ok")     -> constructor
(Module, "io")          -> imported module
(Value, "println")      -> unqualified imported function
```

Rust's name resolution uses the same distinction: different syntactic contexts
resolve names in different namespaces.[^1]

## Project resolution

For whole-project resolution, the compiler first parses and builds ASTs for all
project modules. It then builds a module interface for each one. A module
interface records the public status and span of each exported member.

When another module refers to a qualified member, the resolver checks the target
module interface. If the member exists and is public, the reference can point at
that symbol. If the member is private, the resolver emits a diagnostic at the
use site.

Dependency package resolution is limited today. The project-module path is in
place: imports become module symbols, module symbols can lead to interfaces, and
qualified member references preserve enough information for later passes.

## Why this matters downstream

Type checking should not have to rediscover which `name` a variable means.
Lowering should not have to guess whether `io.println` is a module call or field
access. Code generation should not have to parse source text to find a
function's declaration.

Name resolution gives those passes explicit links:

```text
source name -> symbol -> kind, namespace, span, module visibility
```

That link turns a syntax tree into a program the rest of the compiler can reason
about.

[^1]: Rust Reference, "Name resolution": https://doc.rust-lang.org/beta/reference/names/name-resolution.html
