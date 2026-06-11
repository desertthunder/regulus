# Top-down parsing

Top-down parsers start at the grammar's start symbol and expand productions
downward until they match tokens in the input. The expansion succeeds when
every token is consumed in the right order.

Two styles build on this idea: recursive descent and LL table-driven parsing.
Both work from the top of the grammar and both are defeated by left-recursive
rules, but they encode parsing decisions differently.

## Recursive descent

A recursive descent parser translates each grammar nonterminal into a function.
Parsing an `expression` calls `parse_expression`, which may call `parse_term`,
which calls `parse_factor`, and so on. The call stack mirrors the shape of the
parse tree as it grows.[^1]

For the stratified arithmetic grammar from the previous section:

```text
expression     -> addition
addition       -> multiplication (("+" | "-") multiplication)*
multiplication -> primary (("*" | "/") primary)*
primary        -> number | "(" expression ")"
```

a recursive descent parser has four functions, one per nonterminal. Each
function inspects the current token, chooses a production, and delegates to
other functions. Grammar structure maps directly to code structure, which makes
these parsers legible and relatively easy to debug by hand.

The typical implementation maintains a single lookahead token. When a function
must choose between alternatives, it inspects that token to pick the right
production. If every alternative begins with a distinct token, one lookahead
token is always enough.

### Backtracking

When the first token does not uniquely identify a production, a simple recursive
descent parser can try alternatives in order and restore the input position on
failure. This is a backtracking parser.

Backtracking handles grammars that are not predictable from a fixed number of
tokens, but the cost compounds quickly. A parser that retries the same prefix
through multiple nonterminals can take exponential time on certain inputs.[^2]
Practical parsers avoid this by restructuring grammar rules so that each
alternative begins with a distinct token or token set.

## LL grammars

The notation LL(k) names a class of both grammars and parsers. The first L
means the input is read left to right. The second L means the parser produces a
leftmost derivation: at each step, it expands the leftmost unexpanded
nonterminal. The integer k is the number of lookahead tokens needed to make
each expansion decision without backtracking.[^2]

A grammar is LL(1) when, for every nonterminal and every possible input token,
at most one production can apply. Most production language grammars are written
to be close to LL(1), sometimes with a small number of two-token lookahead
cases.

### FIRST and FOLLOW sets

Two sets guide LL parsing decisions. FIRST(α) is the set of terminals that
can appear at the start of any string derived from α. If α can produce the
empty string, ε is also included.[^2]

```text
FIRST(multiplication) = { number, "(" }
FIRST(primary)        = { number, "(" }
FIRST(addition)       = { number, "(" }
```

FOLLOW(A) is the set of terminals that can appear immediately after nonterminal
A in any valid sentential form. When A can derive ε and the lookahead token is
not in FIRST(A), the parser consults FOLLOW(A) to decide whether the empty
production applies.

```text
FOLLOW(addition)       = { ")", "$" }    -- "$" is end-of-input
FOLLOW(multiplication) = { "+", "-", ")", "$" }
```

An LL(1) parsing table has one row per nonterminal and one column per terminal.
Each cell holds the production to expand. Building the table is mechanical:
for each production A → α, add it to the cell for every token in FIRST(α);
if ε is in FIRST(α), add it to the cell for every token in FOLLOW(A).[^2]

The grammar is LL(1) if and only if every cell holds at most one production.
A conflict means that a single lookahead token is not enough to pick the
right rule.

A table-driven LL(1) parser maintains an explicit stack and consults the table
on each step instead of using the call stack for recursion. It accepts exactly
the same class of languages as a predictive recursive descent parser for the
same grammar. The difference is implementation style, not expressive power.

## Left recursion

A left-recursive rule causes a recursive descent parser to loop forever. With
no token consumed, `parse_expression` calls itself before doing anything else:

```text
expression -> expression "+" term | term
```

Table-driven LL parsers fail on the same grammars for the same reason: the
FIRST sets become circular and the parsing table cannot be constructed.

The standard rewrite moves the recursion to the right using a new nonterminal
for the tail:

```text
expression  -> term expression'
expression' -> "+" term expression' | ε
```

This grammar generates the same language. The parse tree differs: the tail
nonterminal chains to the right instead of the left. If the compiler needs
left-associative trees, it has to reassociate after parsing, or build the node
in reverse order as it unwinds the tail recursion.[^1]

Indirect left recursion, where A derives Aα only through a chain of other
nonterminals, requires a more systematic procedure. The Dragon Book describes
an algorithm that imposes an ordering on nonterminals and eliminates cycles
iteratively.[^2]

## Left factoring

A related problem appears when two productions for the same nonterminal share
a common prefix:

```text
statement -> "if" expression "then" statement
statement -> "if" expression "then" statement "else" statement
```

One lookahead token at the `if` cannot distinguish the two. Left factoring
moves the shared prefix into a single rule and introduces a new nonterminal
for the diverging remainder:

```text
statement  -> "if" expression "then" statement statement'
statement' -> "else" statement | ε
```

The grammar is now LL(1) at this point. Most languages resolve the resulting
dangling-else ambiguity by convention: the `else` binds to the nearest `if`,
which corresponds to always taking the non-ε production for `statement'`.[^2]

## Limitations

LL parsers cannot accept every context-free grammar. Ambiguous grammars have
no LL parsing table at any k because multiple productions would compete for
the same cell. Left-recursive grammars require rewriting. Some grammars need
unbounded lookahead that no fixed k can cover.

Bottom-up parsers handle a broader class of grammars without requiring
left-recursion elimination or left factoring. The cost is that LR parsers are
harder to write by hand and their error reporting is harder to control.

Many production compilers use recursive descent for declarations and control
flow, where the grammar structure is clear, and a separate technique for
expressions. Pratt parsing uses a table of binding powers rather than one
function per precedence level; this eliminates the deep call stack that a
fully stratified recursive descent parser requires and handles left
associativity without grammar rewrites.[^3]

<label for="sn-peg" class="margin-toggle sidenote-number"></label>
<input type="checkbox" id="sn-peg" class="margin-toggle" />
<span class="sidenote">Parsing Expression Grammars (PEGs) formalize
backtracking recursive descent with a prioritized choice operator. PEG parsers
always terminate and never have ambiguity, but the prioritized choice makes
some grammar properties harder to reason about statically. Packrat parsing
achieves linear time for PEGs by memoizing intermediate results.</span>

[^1]: Robert Nystrom, "Representing Code" and "Parsing Expressions," _Crafting Interpreters_: https://craftinginterpreters.com/representing-code.html

[^2]: Alfred V. Aho, Monica S. Lam, Ravi Sethi, and Jeffrey D. Ullman, _Compilers: Principles, Techniques, and Tools_, 2nd ed. (2006), §§ 4.3–4.4 (left recursion, predictive parsing, FIRST/FOLLOW).

[^3]: Vaughan R. Pratt, "Top Down Operator Precedence," _ACM SIGACT-SIGPLAN Symposium on Principles of Programming Languages_, 1973: https://dl.acm.org/doi/10.1145/512927.512931
