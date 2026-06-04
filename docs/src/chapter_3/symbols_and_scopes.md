# Symbols and scopes

A binding introduces a name. A reference uses a name.

In this function, `name` and `message` are bindings:

```gleam
fn greet(name) {
  let message = name
  message
}
```

The final `message` is a reference. It points back to the `let message = ...`
binding. The `name` on the right-hand side of the `let` points back to the
function parameter.

After name resolution, the compiler does not need to ask, "Which `message` is
this?" The reference has been connected to a stable symbol. A symbol is the
compiler's record for a binding: its name, where it was defined, what kind of
binding it is, and a small ID that other data structures can store.

## Symbol tables

Compiler courses often describe this data structure as a symbol table. A symbol
table records information about names, and semantic analysis uses it to check
that references are declared and to connect uses to declarations.[^1]

For a language with nested scopes, the table is a tree or stack of maps rather
than one flat map:

```text
module scope
  functions: main, greet
  imports: io

function scope
  parameters: name
  locals: message
```

When the resolver enters a function or block, it creates a new scope with a
parent pointer. Looking up a name starts in the current scope and then walks to
parents until the name is found or there is nowhere left to look.[^2]

This simple shape is enough for many local names:

```text
define(name, current_scope) -> symbol_id
lookup(name, current_scope) -> symbol_id or error
```

Real languages add more details: namespaces, module paths, imported members,
visibility, prelude names, and type-directed lookups. The compiler is replacing
ambiguous text with explicit links.

## Unknown names

If a reference cannot be found in any visible scope, the compiler reports an
unknown-name error:

```gleam
fn main() {
  missing
}
```

There is no parameter, local variable, imported module, or top-level function
named `missing`, so the reference cannot be connected to a symbol.

The error message should point at the reference rather than the whole function.
That is why syntax tree nodes keep source spans. A span lets the resolver attach
the diagnostic to the exact name that failed to resolve.

## Resolved output

Name resolution should leave later passes with a representation that is harder
to misuse. The compiler starts with the visible name:

```text
Variable("message")
```

After resolution, it can store the symbol link:

```text
Variable("message") -> SymbolId(7)
```

The visible text remains useful for diagnostics and generated output. The
compiler's internal link is the symbol ID. Type checking, lowering, and code
generation can then ask for the symbol's kind, scope, source span, and other
metadata without performing name lookup again.

[^1]: WPI Compiler Design Module 5, "Semantic Analysis Symbol Tables":
    https://web.cs.wpi.edu/~kal/courses/compilers/module5/myst.html
[^2]: University of Wisconsin CS 536, "Symbol Tables and Static Checks":
    https://pages.cs.wisc.edu/~fischer/cs536.s08/course.hold/html/NOTES/6.SYMBOL-TABLES.html
