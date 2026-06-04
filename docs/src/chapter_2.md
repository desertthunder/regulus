# Lexing, parsing, and syntax trees

A compiler needs a structured view of a program before it can do much with it.
Source code begins as text: characters in a file. The compiler turns that text
into shapes it can inspect. A module can contain imports and functions. A
function can contain parameters and a body. A body can contain expressions.

For this project, the path is:

```text
Gleam source
  -> tree-sitter concrete syntax tree
  -> abstract syntax tree
```

A traditional compiler often shows a lexer before the parser:

```text
characters -> tokens -> syntax tree
```

Tree-sitter handles the tokenizing and parsing together for us, but the ideas
are still useful. Lexing explains how source text becomes meaningful pieces.
Parsing explains how those pieces become a program structure. Syntax trees give
later compiler passes a stable representation to inspect.

This chapter covers this process in more detail, including:

- lexing source text into tokens
- parsing tokens into expression structure
- using concrete and abstract syntax trees
- reading the Gleam grammar through tree-sitter
