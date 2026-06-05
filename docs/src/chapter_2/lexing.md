# Lexing

A lexer, sometimes called a scanner, groups characters into tokens. Tokens are
small pieces with a kind and a span in the source file. In Gleam, the text below
contains keywords, identifiers, punctuation, literals, and operators:

```gleam
pub fn add(x: Int, y: Int) -> Int {
  x + y
}
```

A lexer might describe it like this:

```text
pub      keyword
fn       keyword
add      identifier
(        left parenthesis
x        identifier
:        colon
Int      type name
,        comma
y        identifier
:        colon
Int      type name
)        right parenthesis
->       arrow
Int      type name
{        left brace
x        identifier
+        operator
y        identifier
}        right brace
```

Whitespace separates many tokens, although it does not become part of the
program tree. Comments matter to readers and tools, although most compiler
passes do not need them.

<label for="sn-spans" class="margin-toggle sidenote-number"></label>
<input type="checkbox" id="sn-spans" class="margin-toggle" />
<span class="sidenote">A span is often a byte range rather than a line and
column pair. Byte ranges are compact and easy to slice from the original source.
Line and column positions can be computed later for diagnostics.</span>

Spans are the detail a lexer must preserve. If the compiler later rejects `x +
y`, it should point back to the operator or operand that caused the problem, not
just say that something went wrong somewhere in the file.

## Token kinds and token text

Compiler courses often describe lexing as turning an input stream into an
iterator of tokens.[^1] A token has a kind, such as `identifier` or `string`,
and it may also keep the original text or an interpreted value.

Those two pieces are different. The token kind tells the parser what role the
text can play in the grammar. The token text tells later compiler code what the
programmer actually wrote.

```text
source text: "hello\n"
token kind:  string literal
token value: hello followed by a newline
source span: bytes 10..18
```

Identifiers are similar. The parser mostly needs to know that `message` is an
identifier, but name resolution later needs the exact text `message`.

## Regular expressions and lookahead

Many token classes can be described with regular expressions. For example, a
simple integer token might be described as one or more digits:

```text
digit   -> "0" | "1" | ... | "9"
integer -> digit+
```

Real languages need more detail for underscores, bases, floats, strings,
comments, and operators. Lexers also need lookahead because the end of a token
is not always known from the first character. Seeing `=` might mean the token is
`=`, or it might be the start of `==`. Seeing `/` might mean division, the start
of a comment, or part of another operator depending on the language.

Lexer generators use regular-expression specifications to produce scanner code.
Tree-sitter grammars describe tokens and grammar rules together, so this project
does not have a handwritten Gleam lexer. Before the compiler can reason about
functions and expressions, the source text has to be split into meaningful
pieces.

## What this compiler keeps

Tree-sitter handles tokenization internally. This compiler receives a concrete
syntax tree rather than a separate token stream, and every relevant AST node
keeps the important lexer-era information:

- the source text for names, type annotations, literals, and raw syntax
- the tree-sitter kind for raw syntax
- a byte span for diagnostics

That is enough for later passes to report errors precisely without keeping every
whitespace or punctuation token in the AST.

[^1]: Cornell CS 4120, "Lexical Analysis and Regular Expressions": https://www.cs.cornell.edu/courses/cs4120/2022sp/notes/lexing/
