# Bottom-up parsing

Bottom-up parsers work upward from tokens toward the grammar's start
symbol. They recognize when a sequence of symbols on a parse stack
matches the right-hand side of some production and replace it with the
left-hand nonterminal. That replacement is a reduction. The sequence of
reductions, read in reverse, is a rightmost derivation of the input.[^1]

This approach handles left-recursive grammars directly. A rule like
`expression -> expression "+" term` presents no difficulty because
the parser sees both sides before reducing. The LL rewrites described
in the previous section are unnecessary.

## The shift-reduce machine

Every LR parser runs the same four-action loop. A stack interleaves
grammar symbols with parser states. An action table and a goto table
drive decisions.

A shift consumes the next input token and pushes it with the next
state onto the stack. A reduce pops the symbols matching a production's
right-hand side, pushes the left-hand nonterminal, and consults the
goto table for the new state. An accept fires when the augmented start
production is fully reduced and the input is exhausted. Everything
else is an error.[^1]

The state on top of the stack encodes all relevant history. The parser
does not predict a derivation path in advance; it decides one step at
a time by examining what is already on the stack.

## LR items and automaton construction

An LR(0) item is a production with a dot marking the current position.
For the expression grammar from the previous chapter:

```text
expression -> expression "+" term | term
term       -> term "*" factor | factor
factor     -> "(" expression ")" | number
```

`expression -> expression • "+" term` has the dot after the first
symbol: the parser has matched an expression and expects a `+`.
`factor -> "(" expression ")" •` is a complete item, meaning the full
right-hand side is on the stack and a reduction can fire.

Building the automaton starts from the closure of the augmented start
item `[S' -> • S]`. The closure operation expands each item whose dot
precedes a nonterminal B by adding all items `[B -> • gamma]` for every
production of B. This is repeated until no new items appear.[^1]

The goto operation computes the successor state after reading a symbol
X. It collects every item in the current state that has its dot before
X, advances each dot one position, and takes the closure of the result.
States are nodes; goto transitions are labeled edges. The complete
collection of states is the LR(0) canonical automaton.

## Conflicts

A conflict arises when a state and input symbol permit more than one
action. In a shift-reduce conflict, one item calls for shifting while
a completed item also permits reducing on the same token. In a
reduce-reduce conflict, two completed items both want to reduce on the
same lookahead.

LR(0) parsers have no lookahead: they reduce whenever any completed
item appears, regardless of what follows. Shift-reduce conflicts arise
on nearly every realistic grammar, which makes LR(0) useful only as a
theoretical starting point.

## SLR parsing

SLR adds a simple lookahead rule. A completed item `A -> gamma •`
triggers a reduction only when the current input token is in FOLLOW(A),
the set of terminals that can appear immediately after A in any
sentential form.[^1]

The FOLLOW sets are the same ones used to build LL parsing tables.
Restricting reductions to tokens in FOLLOW(A) eliminates many spurious
actions, but FOLLOW sets aggregate context globally: they record every
terminal that can follow A anywhere in the grammar, not only in the
state where the conflict occurs. Some grammars still produce conflicts
that more precise lookahead would eliminate.

## Canonical LR(1)

The full solution embeds a lookahead directly into each item. An LR(1)
item has the form `[A -> alpha • beta, a]`, where `a` is the terminal
that may follow this particular occurrence of A in this context. The
closure operation propagates lookaheads: when adding `[B -> • gamma, b]`,
the lookahead `b` is drawn from FIRST(beta a), the terminals that can
begin the remaining suffix of the enclosing item.[^1]

Two items with identical production and dot position but different
lookaheads become distinct states. This precision eliminates the
conflicts that SLR's global FOLLOW sets cannot, at the cost of
multiplying states. A grammar with a few hundred LR(0) states can
yield several thousand LR(1) states.

Knuth proved in 1965 that every deterministic context-free language
has an LR(k) grammar for some k, and that k = 1 suffices for all
languages of practical interest.[^2]

## LALR parsing

DeRemer observed in 1969 that many distinct LR(1) states share the
same LR(0) core: identical item productions and dot positions, differing
only in their lookahead sets.[^3] Merging those states yields a parser
with as many states as the LR(0) automaton but with more precise
lookaheads than SLR.

After merging, each item carries the union of the lookaheads from all
states that were collapsed. When no merge creates a new reduce-reduce
conflict, the LALR parser accepts exactly the language that the
canonical LR(1) parser accepts. On rare grammars, merging can introduce
reduce-reduce conflicts that the unmerged automaton does not have;
in practice, programming language grammars are designed to avoid this.

LALR(1) is the basis for yacc, Bison, and most production parser
generators. The tables fit in memory even for large grammars, and the
generated parsers run in linear time.

## Limitations

LR parsers detect errors at the first token for which no action exists,
which is the correct position. The error message, however, reflects the
internal state number rather than a human-readable expectation.
Extracting useful diagnostics requires annotating the grammar or
post-processing the tables.[^1]

Error recovery typically works by discarding stack entries and input
tokens until a synchronization point is reached, or by inserting a
placeholder token. Both strategies need per-grammar tuning. Recursive
descent parsers are easier to equip with precise messages because each
function handles one named construct and can report what it expected.

Writing LR grammars by hand is also harder than writing LL grammars.
Parser generators automate table construction, but they report
conflicts as item sets. Reading those requires familiarity with the
internals that LL grammar authors rarely need.

<label for="sn-glr" class="margin-toggle sidenote-number"></label>
<input type="checkbox" id="sn-glr" class="margin-toggle" />
<span class="sidenote">Tomita's GLR algorithm (1984) extends LR parsing
to handle arbitrary context-free grammars, including ambiguous ones, by
forking the parse stack at every conflict and pursuing all alternatives
simultaneously. The cost is potentially cubic time on highly ambiguous
input, though on typical programming language grammars it stays close
to linear.</span>

[^1]: Alfred V. Aho, Monica S. Lam, Ravi Sethi, and Jeffrey D. Ullman, _Compilers: Principles, Techniques, and Tools_, 2nd ed. (2006), §§ 4.5–4.8 (LR items, SLR, canonical LR, LALR, error recovery).

[^2]: Donald E. Knuth, "On the Translation of Languages from Left to Right," _Information and Control_ 8(6), 1965, pp. 607–639. https://doi.org/10.1016/S0019-9958(65)90426-2

[^3]: Frank L. DeRemer, _Practical Translators for LR(k) Languages_, Ph.D. thesis, Massachusetts Institute of Technology, 1969.
