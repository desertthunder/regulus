# Concrete and abstract syntax trees

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
original Gleam code to underline. Lists in the AST keep source order, while
comments, whitespace, and punctuation that no longer carry meaning are left
behind in the concrete tree.

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
