# Concrete and abstract syntax trees

A concrete syntax tree, or CST, stays close to the program as written. It
records which grammar rules matched and where each node appears in the file. It
also keeps syntax details that are useful to parsers and editor tools.

Tree-sitter produces this kind of tree. Its documentation describes syntax nodes
as grammar-rule nodes with source positions stored both as byte offsets and as
row and column points. The tree also keeps anonymous token nodes such as
commas and parentheses, while named nodes represent grammar constructs that are
more useful to analysis.[^1]

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
say: this module imports `gleam/io`; it defines a public function named
`main`; inside the function, it binds `message` and then calls `io.println`.

Compiler texts and lecture notes use this split because later phases need a
program model, not a record of every grammar production. Appel describes ASTs as
a way to avoid tangling syntax and semantics in one compiler phase.[^2] Cornell
notes make the same distinction: a parse tree explains how grammar productions
derive the input, while an AST drops nodes that later compiler stages do not
need.[^3]

The current AST records:

- imports
- functions
- constants
- external functions and types
- type aliases and type definitions
- attributes and target groups
- parameters
- type annotations
- blocks
- `let` bindings
- literals
- variables
- function calls
- field access
- simple `case` expressions
- binary, unary, and pipeline expressions
- tuples, lists, records, record updates, bit arrays, and captures
- anonymous functions, `use`, `assert`, `todo`, `panic`, and `echo`
- tuple, list, constructor, alias, discard, literal, and raw patterns

Every AST node keeps a source span. A span is a byte range inside the source
file. If the compiler reports an error, the span tells it which part of the
original Gleam code to underline. Lists in the AST keep source order, while
comments, whitespace, and punctuation that no longer carry meaning are left
behind in the concrete tree.

At this point, names are text plus source locations. If the source says
`message`, the AST stores the text `message` and where it appeared. Deciding
which binding that name refers to is a separate job.

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

## What stays concrete

Concrete syntax keeps details that are meaningful to source tools even when the
compiler does not need them after parsing:

- punctuation and delimiters
- grammar helper nodes such as argument lists and parameter lists
- comments and whitespace-sensitive source layout
- exact node kinds produced by the grammar
- error and missing nodes used during recovery

Those details are not useless. They are how a parser explains the source text.
They are also how an editor can highlight syntax, select syntax nodes, or keep
working while the user has half-written code.

Tree-sitter's named-node APIs are a bridge between CST and AST work. Traversing
only named children makes the tree feel closer to an AST, but it is still a
tree-sitter tree. The compiler still builds typed Rust data because later phases
need domain-specific variants such as `Expression::Call`, `Pattern::Alias`, and
`Declaration::TypeDefinition`.

## What belongs in the AST

An AST should keep information that later compiler phases need to answer
language questions:

- which declarations exist in the module
- which declarations are public
- which names, labels, and type annotation text were written
- how expressions and patterns nest
- which children appear in which source order
- where each meaningful construct came from in the source file

It should not answer every question immediately. In this compiler, the AST does
not decide which binding a variable refers to, whether a type exists, or whether
a field access is valid. Those checks belong to name resolution and type
checking.

This boundary keeps parsing explicit. The AST can represent `io.println(x)` as
a field access used as a call target. Resolution can later decide whether `io`
is an imported module. Type checking can later decide whether a non-module
field access is valid for the record type being inspected.

## Building an AST

Parser generators can attach actions to grammar productions, and those actions
often build AST nodes as syntax is recognized.[^3] This project uses two steps:

```text
tree-sitter CST
  -> AST builder
  -> compiler-owned AST
```

The AST builder walks named tree-sitter nodes and converts each recognized Gleam
construct into a Rust data structure. For example, an `import` node becomes an
`Import` with a module name, optional alias, and unqualified imports. A
`function` node becomes a `Function` with visibility, name, parameters, return
annotation, and body.

The walk is intentionally mechanical:

1. Match the tree-sitter node kind.
2. Read important children by field name where the grammar provides one.
3. Convert those children into typed AST structs and enums.
4. Copy the source text for names, literals, and type annotations.
5. Store a byte span on the AST node.
6. Preserve ordered child lists for declarations, statements, arguments,
   expression elements, and pattern elements.

Field names matter because they make the conversion less dependent on child
position. Tree-sitter exposes named fields through APIs such as
`child_by_field_name`, which lets the builder ask for a function's `name`,
`parameters`, `return_type`, and `body` instead of counting children.[^1]

This compiler-owned AST is deliberately smaller than the CST. It keeps the
information that later diagnostics need. Every node that can produce an error
keeps a span.

## Declarations and executable syntax

The AST models declarations separately from executable expressions. A module has
a source-order `declarations` list, plus convenience lists for common lookup
paths such as `imports` and `functions`.

That split matters because declarations and expressions are consumed
differently:

- imports affect name resolution for the whole module
- type declarations and aliases contribute interface data
- external declarations describe host or foreign functions
- functions contain executable blocks that later lower to IR
- attributes and target groups influence compilation context

Keeping declarations explicit avoids treating every top-level item as a generic
statement. It also lets later passes collect module-level facts before checking
function bodies.

Executable syntax is modeled under `Expression`, `Statement`, and `Pattern`.
Nested expressions and patterns use boxes and vectors where the source can nest
or repeat. For example, a call stores its callee expression and an ordered list
of arguments. A `case` stores ordered subjects and ordered clauses. A list
pattern stores ordered elements and an optional tail.

Order is part of the program. It controls evaluation order, binding order, and
the order in which diagnostics should be reported. The AST should simplify
grammar shape, not reorder the user's program.

## Spans connect the trees

The compiler does not keep tree-sitter nodes in the AST, but every AST node
that may need a diagnostic keeps a span copied from the CST node. A span is a
file id plus a byte range. That small piece of source identity lets later passes
report errors without depending on tree-sitter.

This is why AST design affects all later phases. If the AST forgets the span of
a pattern, the exhaustiveness checker can know that a pattern is invalid but not
where to underline. If it forgets the order of expressions, IR lowering has to
reconstruct evaluation order. If it stores declarations as undifferentiated raw
text, name resolution has to parse them again.

The AST is therefore the compiler's source model. It is still syntax, not
semantics, but it is syntax shaped for compiler passes rather than for grammar
debugging.

## Raw syntax

The AST does not need executable support for every Gleam feature before the
parser can accept real modules. For syntax that is recognized by tree-sitter but
not yet lowered or type checked, the AST can store a raw syntax node:

```text
RawSyntax {
  kind: "type_definition",
  source: "pub type User { ... }",
  span: bytes 40..92,
}
```

Raw syntax lets the compiler preserve source order and spans while support grows
feature by feature. Later passes can either handle the raw form, report a
targeted unsupported-feature diagnostic, or use it as module-interface data.

This helps with top-level Gleam syntax such as constants, type definitions,
type aliases, external declarations, attributes, and target groups. The parser
and AST builder can keep them in the module instead of pretending they do not
exist.

## Source order

The AST keeps declarations and statements in source order, even when a later
pass is allowed to collect some declarations before checking bodies. Diagnostics
should follow the user's source order, and generated dumps are easier to read
when they resemble the input module.

The AST also keeps separate lists for imports and functions because many passes
want direct access to those common declarations. This is a convenience view over
the same source module, not a second language.

[^1]: Tree-sitter documentation, "Basic Parsing": https://tree-sitter.github.io/tree-sitter/using-parsers/2-basic-parsing.html

[^2]: Andrew W. Appel, _Modern Compiler Implementation in C_, preface: https://www.cs.princeton.edu/~appel/modern/c/preface.html

[^3]: Cornell CS 4120, "Building ASTs and Handling Errors": https://www.cs.cornell.edu/courses/cs4120/2022sp/notes.html?id=ast
