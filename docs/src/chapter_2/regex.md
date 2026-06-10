# Regular expressions

A regular expression is a notation for describing a set of strings over an
alphabet. Lexer specifications use them to name token classes: an integer
is one or more digits, an identifier starts with a letter, a comment runs
from `//` to the end of the line.

## Primitives and operators

Three primitive forms start the grammar. The empty string ε matches the
string of zero characters. A single character `a` matches exactly that
character. Parentheses group sub-expressions without changing the language
they describe.[^1]

Three operators build larger expressions from smaller ones. Concatenation
`rs` matches any string formed by a string from `r` followed by a string
from `s`. Alternation `r | s` matches any string from `r` or any string
from `s`. Kleene star `r*` matches zero or more strings from `r`
concatenated in sequence.[^1]

Useful shorthand follows from these. `r+` means one or more occurrences:
`rr*`. `r?` means optional: `r | ε`. A character class `[abc]` stands
for `a | b | c`, and a range `[a-z]` covers all characters between `a`
and `z` in encoding order.

## Token patterns

Token classes for a typical language can be specified concisely:

```text
digit   ->  [0-9]
letter  ->  [a-zA-Z_]
integer ->  digit+
ident   ->  letter (letter | digit)*
float   ->  digit+ "." digit+ (("e" | "E") ("+" | "-")? digit+)?
string  ->  '"' ([^"\\] | '\\' .)* '"'
comment ->  "//" [^\n]* "\n"
```

The float pattern shows nesting: the optional exponent is a group with
an optional sign. The string pattern uses `[^"\\]` for any character that
is not a quote or backslash, then `\\` followed by any character for an
escape sequence.

## Formal semantics

A regular expression `r` defines a language L(r): the set of strings it
matches.[^1] The rules compose inductively:

```text
L(ε)     =  { "" }
L(a)     =  { "a" }
L(rs)    =  { xy | x in L(r), y in L(s) }
L(r|s)   =  L(r) ∪ L(s)
L(r*)    =  L(ε) ∪ L(r) ∪ L(rr) ∪ L(rrr) ∪ ...
```

A language is regular when some regular expression describes it. The class
of regular languages is closed under union, concatenation, and Kleene star:
applying any of those operators to regular languages always yields another
regular language.

## Limits

Regular expressions cannot describe all token-like structures. Balanced
delimiters require counting: a language of strings with equal numbers of
`(` and `)` characters has no regular description. Comments that nest also
exceed what a regular expression can match.[^2]

Backreferences in PCRE-style engines, where a pattern refers back to what
a group matched earlier, exceed the regular language class entirely. A
lexer based on theoretical regular expressions cannot match `aXa` where
the same string `X` appears on both sides. Practical lexer specifications
avoid needing this.

## Connection to automata

Every regular expression compiles to a finite-state machine. The connection
runs through Thompson's construction, which converts a regular expression
to an NFA, and the subset construction, which converts the NFA to a DFA.
The reverse direction also holds: every DFA has an equivalent regular
expression, though the expression can be much longer than the
automaton.[^1] Both constructions are covered in the previous section on
finite-state machines.

[^1]: Alfred V. Aho, Monica S. Lam, Ravi Sethi, and Jeffrey D. Ullman, _Compilers: Principles, Techniques, and Tools_, 2nd ed. (2006), §§ 3.3–3.5 (regular expressions, their formal semantics, and the connection to finite automata).

[^2]: John E. Hopcroft, Rajeev Motwani, and Jeffrey D. Ullman, _Introduction to Automata Theory, Languages, and Computation_, 3rd ed. (2006), § 4.2 (pumping lemma for regular languages).
