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

Crafting Interpreters uses this problem to introduce precedence and associativity
in expression parsing.[^1] Precedence decides which operator binds more tightly:
`*` should group before `+`. Associativity decides how operators at the same
precedence group: `a - b - c` usually means `(a - b) - c`.

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

This matters for a compiler because later passes should not have to guess what
the source meant. Type checking, lowering, and code generation all rely on the
parser to choose one tree.

<label for="sn-pratt" class="margin-toggle sidenote-number"></label>
<input type="checkbox" id="sn-pratt" class="margin-toggle" />
<span class="sidenote">Many compilers parse expressions with a Pratt parser or
precedence-climbing parser instead of writing one function per precedence level.
The data structure produced is the same kind of tree; the implementation is just
more compact for languages with many operators.</span>

[^1]: Robert Nystrom, "Parsing Expressions," Crafting Interpreters: https://craftinginterpreters.com/parsing-expressions.html
