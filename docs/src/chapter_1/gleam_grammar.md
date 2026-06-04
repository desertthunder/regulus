# Gleam's grammar

Tree-sitter is an incremental parsing system used by editors, code browsers, and
language tools.[^1] It is a good fit here because it gives us a real Gleam
parser without requiring this project to implement a lexer and parser first.

The `tree-sitter-gleam` crate embeds the Gleam grammar and exposes it through
the Rust `tree-sitter` API.[^2] Its README states that the grammar can parse the
entire Gleam language and is largely based on Gleam's own parser.[^3]

Tree-sitter also represents syntax errors in the tree. That means malformed
source can still produce a tree, but that tree contains error nodes. This
compiler checks for those nodes before building the AST and reports a diagnostic
at the first error span.

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

This compiler currently accepts imports and functions at the top level. Other
forms may parse correctly with tree-sitter, but they are reported as unsupported
when building the AST.

A Gleam function node has named fields for its name, parameters, optional return
type, and body:

```text
function
  name: identifier
  parameters: function_parameters
  return_type: type?
  body: function_body
```

Imports have a similarly direct shape. The grammar represents
`import gleam/list as list` as an `import` node with a `module` field and an
optional `alias` field.

Some nodes are less obvious. The `tree-sitter-gleam` README notes that a
`function_call` can call more than a plain identifier.[^3] In
`io.println("hi")`, the parser sees a field access followed by a call. The
parser cannot know from syntax alone whether `io` is a module or a value; that
is decided after parsing.

Gleam constructors such as `True`, `False`, and `Nil` are parsed as record-like
syntax by the grammar. The current AST treats those names as bool and nil
literals. Other record forms are not accepted yet.

[^1]: Tree-sitter documentation, "Introduction": https://tree-sitter.github.io/tree-sitter/
[^2]: `tree-sitter` Rust crate documentation: https://docs.rs/tree-sitter
[^3]: `tree-sitter-gleam` README: https://github.com/gleam-lang/tree-sitter-gleam
[^4]: `tree-sitter-gleam` grammar source, `source_file` and module statements: https://github.com/gleam-lang/tree-sitter-gleam/blob/main/grammar.js
