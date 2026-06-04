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

Whitespace separates many tokens, but it usually does not become part of the
program tree. Comments are similar: they matter to readers and tools, but most
compiler passes do not need them.

<label for="sn-spans" class="margin-toggle sidenote-number"></label>
<input type="checkbox" id="sn-spans" class="margin-toggle" />
<span class="sidenote">A span is usually a byte range, not a line and column
pair. Byte ranges are compact and easy to slice from the original source. Line
and column positions can be computed later for diagnostics.</span>

Spans are the detail a lexer must preserve. If the compiler later rejects `x +
y`, it should point back to the operator or operand that caused the problem, not
just say that something went wrong somewhere in the file.
