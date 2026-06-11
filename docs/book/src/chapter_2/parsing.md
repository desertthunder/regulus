# Parsing

A parser checks whether tokens fit the language grammar. If they do, it builds a
syntax tree. A small grammar for arithmetic expressions might say:

```text
expression -> literal
expression -> identifier
expression -> expression "+" expression
expression -> "(" expression ")"
```

This is enough to describe examples like `1`, `x`, `1 + 2`, and `(1 + 2)`, but
it has a problem. It does not say how to group `1 + 2 * 3`.

Crafting Interpreters uses this problem to introduce precedence and
associativity in expression parsing.[^1] Precedence decides which operator
binds more tightly: `*` should group before `+`. Associativity decides how
operators at the same precedence group: in many languages, `a - b - c` means
`(a - b) - c`.

A parser can encode those rules by splitting `expression` into layers:

```text
expression     -> addition
addition       -> multiplication (("+" | "-") multiplication)*
multiplication -> primary (("*" | "/") primary)*
primary        -> literal | identifier | "(" expression ")"
```

With these layers, `1 + 2 * 3` has only one intended shape:

```text
+
├─ 1
└─ *
   ├─ 2
   └─ 3
```

Later passes need one tree for the source. Type checking, lowering, and code
generation all rely on the parser to choose that tree.

<label for="sn-pratt" class="margin-toggle sidenote-number"></label>
<input type="checkbox" id="sn-pratt" class="margin-toggle" />
<span class="sidenote">Many compilers parse expressions with a Pratt parser or
precedence-climbing parser instead of writing one function per precedence level.
The data structure produced is the same kind of tree; the implementation is just
more compact for languages with many operators.</span>

## Context-free grammars

Regular expressions work well for tokens. Most programming language syntax needs
more structure. Balanced parentheses are the classic example: a parser needs to
remember how many groups have been opened, and that requires more structure than
a finite automaton can provide.[^2]

Context-free grammars describe syntax with productions:

```text
function -> "fn" identifier parameters block
block    -> "{" statement* "}"
```

The names on the left, such as `function` and `block`, are nonterminals. They
stand for larger syntax categories. The quoted pieces are terminals: the tokens
that appear in the source.

Parsing checks whether a sequence of tokens can be derived from the grammar. If
it can, the parser builds a tree that records the chosen structure.

## Parsing is not semantic analysis

The parser decides whether the source has the right syntactic shape. It does not
decide whether every name exists or every expression has the right type.

```gleam
fn main() {
  missing(1)
}
```

This is valid syntax. It has a function, a block, and a call expression. The
parser can build a tree for it. The unknown name `missing` is reported later by
name resolution.

Type checking has the same boundary:

```gleam
fn main() {
  "one" + 2
}
```

The expression is shaped like an operator expression. Whether the operands are
valid for that operator is a type-checking question.

## Errors and recovery

A parser can fail as soon as it sees malformed syntax. Editor-oriented parsers
often try to produce a tree anyway, so they can support syntax highlighting,
code navigation, and diagnostics while a user is typing.

Tree-sitter is designed for that use case. It builds concrete syntax trees
incrementally and can represent syntax errors in the tree.[^3] This compiler
checks the tree for error or missing nodes before building the AST. If it finds
one, it reports a parse diagnostic at the first error span.

That keeps later compiler passes simpler. They can assume the AST was built from
syntax that tree-sitter accepted, and users get a source location for parse
errors.

[^1]: Robert Nystrom, "Parsing Expressions," Crafting Interpreters: https://craftinginterpreters.com/parsing-expressions.html

[^2]: Cornell CS 4120, "Context-Free Grammars": https://www.cs.cornell.edu/courses/cs4120/2022sp/notes/grammars/

[^3]: Tree-sitter documentation, "Introduction": https://tree-sitter.github.io/tree-sitter/
