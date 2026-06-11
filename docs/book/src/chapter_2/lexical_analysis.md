# Lexical analysis

Lexical analysis is the first phase of a compiler's front end. It reads a
character stream and produces a token stream. Tokens are the named pieces
that a parser reasons about: keywords, identifiers, numbers, operators,
punctuation.

The separation between character-level work and grammar-level work is
deliberate. A finite-state machine can recognize tokens from character
sequences efficiently; context-free grammars are more powerful but also
more expensive to run. Handling each layer with the right tool keeps the
overall design simpler.[^1]

## The character-to-token boundary

A lexer consumes the source file character by character, grouping runs
into tokens. At each position it finds the longest sequence that matches
any token class, emits that token, and advances past it. This longest-match
rule ensures that `==` becomes a single equality token rather than two
consecutive assignment tokens.

Token classes are typically described as regular expressions and compiled
to a DFA. The DFA runs on the character stream, advancing one character at
a time. When no transition is possible and the current state accepts, a
token is complete.

## Why parsers do not work on characters

A parser could, in principle, work directly on characters. Context-free
grammars can describe character sequences as easily as token sequences.
In practice the token layer reduces the grammar's size, shrinks parsing
tables, and separates concerns that change at different rates. Whitespace
handling, comment skipping, and string escape sequences belong to the
character layer. Operator precedence and statement structure belong to the
token layer.[^1]

## Foundations and algorithms

Regular expressions are the notation for token class specifications.
Finite-state machines are the automata that execute them. The connection
between the two — Thompson's construction and the subset construction —
explains why lexer generators work.

Parsing covers the next layer: how a token sequence is checked against a
grammar and shaped into a tree. Top-down and bottom-up algorithms approach
this from different directions and involve different implementation costs.

[^1]: Alfred V. Aho, Monica S. Lam, Ravi Sethi, and Jeffrey D. Ullman, _Compilers: Principles, Techniques, and Tools_, 2nd ed. (2006), § 3.1 (the role of the lexical analyzer, the separation of lexing and parsing).
