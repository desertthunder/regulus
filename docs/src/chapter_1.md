# Syntax trees and Gleam

A compiler needs a structured view of a program before it can do much with it.
Source code begins as text: characters in a file. A syntax tree gives that text
shape. A module can contain imports and functions. A function can contain
parameters and a body. A body can contain expressions.

For this project, the first two shapes are:

```text
Gleam source
  -> tree-sitter concrete syntax tree
  -> abstract syntax tree
```

The concrete tree is produced by tree-sitter. The abstract tree is the smaller
shape this compiler works with after parsing.

## Concrete syntax trees

A concrete syntax tree, or CST, stays close to the program as written. It records
which grammar rules matched and where each node appears in the file. It also
keeps syntax details that are useful to parsers and editor tools.

For this Gleam program:

```gleam
import gleam/io

pub fn main() {
  let message = "hello"
  io.println(message)
}
```

tree-sitter produces a tree shaped roughly like this:

```text
source_file
  import
    module
  function
    visibility_modifier
    identifier
    function_parameters
    function_body
      let
        identifier
        string
      function_call
        field_access
          identifier
          label
        arguments
          argument
            identifier
```

The CST mirrors the grammar. Nodes such as `function_parameters`, `arguments`,
and `visibility_modifier` are helpful because they describe exactly how the text
matched Gleam syntax.

## Abstract syntax trees

An abstract syntax tree, or AST, keeps the parts of syntax that carry meaning
for the compiler and removes parser details. For the example above, the AST can
say: this module imports `gleam/io`; it defines a public function named `main`;
inside the function, it binds `message` and then calls `io.println`.

The current AST records:

- imports
- functions
- parameters
- type annotations
- blocks
- `let` bindings
- literals
- variables
- function calls
- field access
- simple `case` expressions

Every AST node keeps a source span. A span is a byte range inside the source
file. If the compiler reports an error, the span tells it which part of the
original Gleam code to underline.

Names are still just names at this point. If the source says `message`, the AST
stores the text `message` and where it appeared. Deciding which binding that
name refers to is a separate job.

## Concrete vs abstract syntax trees

A CST answers, "How did this source text match the grammar?" An AST answers,
"What program structure should the compiler work with?"

For example, the CST for a function call needs nodes for the parentheses and the
argument list because those are part of the grammar. The AST can store the same
idea as a call expression with a function and a list of arguments. Both are
correct, but they serve different readers.

```text
CST: function_call -> arguments -> argument -> identifier
AST: Call { function, arguments }
```

The CST is best when checking parser behavior, showing syntax errors, or
understanding the exact grammar shape. The AST is better once the compiler wants
to reason about declarations, expressions, and source spans without carrying
every bit of punctuation along.

This project keeps both views. Tree-sitter gives us the CST. The compiler then
builds an AST that is smaller and easier to use for the rest of the work.

## Why tree-sitter

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

## Gleam's grammar

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
