# Lexing, parsing, and syntax trees

A compiler needs a structured view of a program before it can do much with it.
Source code begins as text: characters in a file. The compiler turns that text
into shapes it can inspect.

For this project, the path is:

```text
Gleam source
  -> tree-sitter concrete syntax tree
  -> abstract syntax tree
  -> resolver, type checker, and IR lowering
```

A traditional compiler separates the first two steps more explicitly:

```text
characters -> tokens -> syntax tree
```

Tree-sitter handles tokenizing and parsing together, but the underlying
stages remain useful to understand. The theory helps explain why certain
grammars are difficult to parse, how token classes are compiled to
efficient matchers, and what tradeoffs different parsing algorithms make.

The chapter covers this ground in two parts. The first is lexical
analysis: how a character stream becomes a token stream. Regular
expressions describe token classes formally. Finite-state machines run
them efficiently. The connection between the two — Thompson's construction
and subset construction — is why lexer generators work.

The second part is parsing. Context-free grammars name the structure a
flat token sequence should have. Top-down parsers predict and expand from
the start symbol; bottom-up parsers recognize and reduce toward it. Both
families have practical tradeoffs in power, speed, and error recovery.

The chapter closes with syntax trees. A concrete syntax tree mirrors the
grammar; an abstract syntax tree keeps the structure that later compiler
phases need. This compiler maintains both. Tree-sitter produces the
concrete tree. The compiler builds the abstract one.[^1]

[^1]: Tree-sitter documentation, "Introduction": https://tree-sitter.github.io/tree-sitter/
