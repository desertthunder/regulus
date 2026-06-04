# Gleam's grammar

Tree-sitter is an incremental parsing system used by editors, code browsers, and
language tools.[^1] It lets this project use an existing Gleam parser before
implementing its own lexer and parser.

The `tree-sitter-gleam` crate embeds the Gleam grammar and exposes it through
the Rust `tree-sitter` API.[^2] Its README states that the grammar can parse the
entire Gleam language and is largely based on Gleam's own parser.[^3]

Tree-sitter also represents syntax errors in the tree. Malformed source can
produce a tree that contains error nodes. This compiler checks for those nodes
before building the AST and reports a diagnostic at the first error span.

You can inspect the tree-sitter shape of a snippet from Rust with:

```rust
use compiler_core::{parse, source::{SourceFile, SourceFileId}};

let source = SourceFile::new(
    SourceFileId(0),
    "import gleam/io\npub fn main() { io.println(\"hi\") }",
);
let cst = parse::parse(source).expect("parse Gleam source");

println!("{}", cst.tree.root_node().to_sexp());
```

The printed tree is useful when adding support for a new bit of Gleam syntax.
It shows the exact node names and fields that the AST conversion code needs to
read.

The tree-sitter Gleam grammar has a `source_file` root. At the top level, Gleam
can contain imports, constants, external declarations, functions, type
definitions, type aliases, attributes, and target groups.[^4]

This compiler accepts imports and functions at the top level. Other forms may
parse correctly with tree-sitter, then be reported as unsupported when building
the AST.

A Gleam function node has named fields for its name, parameters, optional return
type, and body:

```text
function
  name: identifier
  parameters: function_parameters
  return_type: type?
  body: function_body
```

Imports are direct too. The grammar represents `import gleam/list as list` as an
`import` node with a `module` field and an optional `alias` field.

Some nodes need more interpretation. The `tree-sitter-gleam` README notes that a
`function_call` can call expressions beyond plain identifiers.[^3] In
`io.println("hi")`, the parser sees a field access followed by a call. The
parser cannot know from syntax alone whether `io` is a module or a value; that
is decided after parsing.

Gleam constructors such as `True`, `False`, and `Nil` are parsed as record-like
syntax by the grammar. The current AST treats those names as bool and nil
literals. Other record forms are not accepted yet.

## Grammar shape vs compiler meaning

The tree-sitter grammar describes syntax, not the full meaning of the program.
That is why some tree-sitter shapes need interpretation when the AST is built.

For example, Gleam uses capitalized names for types and constructors. The parser
can distinguish `identifier` from `type_identifier`, but it cannot decide
whether a constructor is public, whether a type is opaque, or whether a
qualified constructor comes from an imported module. Those are later compiler
questions.

The same is true for dotted syntax:

```gleam
io.println("hi")
user.name
```

Both examples involve a field-access shape in the CST. In the first program,
name resolution can discover that `io` is an imported module. In the second,
type checking may need to know the record type of `user` before validating the
field. The parser only records the surface shape.

## Current parser and AST coverage

The current parser accepts Gleam source through tree-sitter and rejects
tree-sitter error nodes before AST construction. The AST builder has explicit
nodes for:

- imports, including aliases and unqualified imports
- functions, parameters, return annotations, and blocks
- literals, variables, calls, field access, and simple `case`
- `let` and `let assert`
- tuple, list, constructor, alias, discard, and literal patterns
- broader top-level Gleam declarations as raw syntax

Names remain textual until name resolution. Type annotations remain source text
until type checking parses and interprets them. This keeps parsing focused on
syntax and avoids mixing later compiler questions into the AST builder.

## Inspecting grammar changes

When adding a new Gleam feature:

1. Parse a small source snippet with tree-sitter.
2. Inspect the `to_sexp()` output.
3. Add or update the AST builder conversion for the relevant node kinds.
4. Preserve spans on the new AST nodes.
5. Add fixtures or snapshots that show the accepted syntax.

This keeps the compiler tied to the actual Gleam grammar and gives later passes
a compact AST built for this project.

[^1]: Tree-sitter documentation, "Introduction": https://tree-sitter.github.io/tree-sitter/
[^2]: `tree-sitter` Rust crate documentation: https://docs.rs/tree-sitter
[^3]: `tree-sitter-gleam` README: https://github.com/gleam-lang/tree-sitter-gleam
[^4]: `tree-sitter-gleam` grammar source, `source_file` and module statements:
    https://github.com/gleam-lang/tree-sitter-gleam/blob/main/grammar.js
