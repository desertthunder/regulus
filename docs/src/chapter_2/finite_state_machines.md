# Finite-state machines

A finite-state machine has a fixed set of states, an alphabet of input
symbols, and a transition function that maps a state and a symbol to a
next state. One state is the start. Some states are accepting. The
machine accepts a string when reading it from the start leaves the machine
in an accepting state.[^1]

Lexers use finite-state machines to recognize tokens. A rule like
"an identifier is a letter followed by zero or more letters or digits"
describes a set of strings. A finite-state machine decides whether any
string belongs to that set one character at a time, with no backtracking.

## Deterministic automata

In a deterministic finite automaton (DFA), every state has exactly one
outgoing transition for each input symbol. Given a state and a character,
the next state is uniquely determined.

For an identifier that starts with a letter and continues with letters
or digits:

```text
states: start, ident, dead
transitions:
  start --letter--> ident
  start --digit-->  dead
  ident --letter--> ident
  ident --digit-->  ident
  ident --other-->  dead
  dead  --any-->    dead
accepting: { ident }
```

Reading `add1` begins at `start`, moves to `ident` on `a`, stays through
`d`, `d`, and `1`, and ends in `ident`, which accepts. Reading `1add` moves
to `dead` on `1` and stays there. The machine rejects.

DFAs are fast to execute. At each character, one table lookup gives the
next state. When the input ends, one check on the final state determines
the result.

## Nondeterministic automata

A nondeterministic finite automaton (NFA) relaxes the uniqueness
requirement. A state can have zero, one, or many transitions on the same
symbol. It can also have epsilon transitions: moves that consume no input
and change state freely.

An NFA accepts a string when any path through the machine from start to
an accepting state exists. The machine does not need to know which path
succeeds; the acceptance condition is existential.

Epsilon transitions are useful when composing automata. Connecting the
accept state of one NFA to the start state of another by an epsilon
transition forms the concatenation of both patterns without restructuring
either fragment.

## Kleene's theorem

A regular language is any language that a finite-state machine can
recognize. Kleene showed that this class is exactly the class described
by regular expressions: for every regular expression there is an NFA
that accepts the same language, and for every NFA there is an equivalent
DFA.[^2]

This equivalence is the theoretical basis for lexer generators. A lexer
specification written in regular expressions can be compiled mechanically
to a DFA that executes in constant time per input character.

## Thompson's construction

Thompson described a procedure for building an NFA from a regular
expression by structural recursion.[^3] Each base case and each operator
produces a small NFA fragment with one start state and one accept state.
The fragments compose without modification.

For a single character `a`, the fragment is:

```text
start --a--> accept
```

For concatenation `AB`, an epsilon transition joins N(A)'s accept state
to N(B)'s start state. The combined fragment spans from N(A)'s start to
N(B)'s accept:

```text
[N(A)] --ε--> [N(B)]
```

For alternation `A | B`, a new start state branches via epsilon into
both fragments, and a new accept state collects their endpoints:

```text
new-start --ε--> [N(A)] --ε--> new-accept
new-start --ε--> [N(B)] --ε--> new-accept
```

For Kleene star `A*`, a new start and accept state surround N(A). An
epsilon from the new start reaches both N(A)'s start and new-accept
directly, allowing zero repetitions. An epsilon from N(A)'s accept
returns to N(A)'s start for additional repetitions, and another reaches
new-accept to stop:

```text
new-start --ε--> N(A).start
new-start --ε--> new-accept
N(A).accept --ε--> N(A).start
N(A).accept --ε--> new-accept
```

Any regular expression can be reduced to these cases. The resulting NFA
has at most twice as many states as the expression has operators.[^1]

## Subset construction

The NFA produced by Thompson's construction can have epsilon transitions
and multiple successors for the same symbol. A DFA cannot. The subset
construction converts the NFA to an equivalent DFA.[^1]

The key concept is the epsilon closure of a state: the set of all states
reachable from it using only epsilon transitions, including itself. For
the start state of the NFA, the epsilon closure is the start state of
the DFA.

Each DFA state is a set of NFA states. Given a DFA state S and a symbol
`a`, the successor DFA state is the epsilon closure of every NFA state
reachable from any state in S by reading `a`. New DFA states are added
as they are discovered. A DFA state is accepting if any NFA state it
contains is accepting.

The number of DFA states is bounded by the power set of NFA states, but
for the NFAs that arise from typical lexer patterns the actual count is
much smaller. In the worst case the construction can produce exponentially
many states; in practice, most token patterns produce modest DFAs.[^1]

<label for="sn-min" class="margin-toggle sidenote-number"></label>
<input type="checkbox" id="sn-min" class="margin-toggle" />
<span class="sidenote">The DFA produced by subset construction may not
be minimal. Hopcroft's algorithm (1971) partitions the states into
equivalence classes and merges those that are indistinguishable by any
future input, yielding a unique minimal DFA for each regular language.
Lexer generators apply this step to reduce table size.</span>

## Connection to lexers

A real lexer recognizes several token classes at once, not just one
pattern. Each class is compiled to an NFA fragment. The fragments are
joined by epsilon transitions from a common start state, and the combined
NFA is converted to a single DFA by subset construction.

The DFA then runs on source text. At each position, it reads characters
until no transition is possible. If the machine is in an accepting state,
the longest matching token is returned; if not, the lexer reports an
error. This longest-match rule, sometimes called maximal munch, ensures
that `>=` is a single token rather than `>` followed by `=`.[^1]

Lexer generators such as `flex` and `re2c` automate this pipeline. Most
practical lexers are the DFAs that result from it, not handwritten code
that simulates state transitions manually.

[^1]: Alfred V. Aho, Monica S. Lam, Ravi Sethi, and Jeffrey D. Ullman,
_Compilers: Principles, Techniques, and Tools_, 2nd ed. (2006), §§ 3.3–3.7
(regular expressions, NFAs, DFAs, Thompson's construction, subset
construction).

[^2]: Stephen C. Kleene, "Representation of Events in Nerve Nets and
Finite Automata," in Claude E. Shannon and John McCarthy, eds.,
_Automata Studies_, Princeton University Press, 1956, pp. 3–42.

[^3]: Ken Thompson, "Regular Expression Search Algorithm,"
_Communications of the ACM_ 11(6), 1968, pp. 419–422.
https://dl.acm.org/doi/10.1145/363347.363387
