# Intermediate representations and lowering

A compiler usually does not translate source code directly into machine code or
WebAssembly. It first translates the program into one or more intermediate
representations, often shortened to IR. An IR is a compiler-friendly form of the
program.[^1]

The source language is designed for people. The target language is designed for
a machine or runtime. An IR sits between them. It gives the compiler a place to
make evaluation order explicit, remove surface syntax, and use data structures
that are easier to analyze or emit.

## Why use an IR

Consider a small Gleam function:

```gleam
fn id(x: Int) -> Int {
  let y = x
  y
}
```

The syntax tree tells us this is a function with a parameter, a `let` binding,
and a final expression. The IR can record a simpler sequence:

```text
function id(x: Int) -> Int
  local y: Int
  y = get x
  return get y
```

This form is less concerned with Gleam syntax and more concerned with the work
that must happen when the program runs. The local variable is explicit. The
assignment is explicit. The final value is explicit. Values in this IR are typed,
and variable references have already been turned into known locals or functions.

IRs come in many forms. Some look like trees. Some look like instruction lists.
Some use control-flow graphs. LLVM bitcode, for example, is a serialized form of
LLVM IR that stores modules, functions, types, constants, and instructions in a
compact binary format.[^2]

The right IR depends on what the compiler needs to do. Ray Toal's notes on
intermediate representations show several common choices, including syntax
trees, three-address code, and stack-machine code.[^3]

## Lowering

Lowering is the act of translating from a higher-level representation into a
lower-level one. Matt Warren describes lowering in the C# compiler as converting
rich language constructs into simpler forms that later parts of the compiler can
handle more easily.[^4]

For this project, lowering means translating typed Gleam AST into core IR.
Gleam has source-level ideas such as `let`, blocks, and `case`. Core IR keeps the
same program meaning, but stores it in a smaller set of constructs:

- functions
- locals
- local reads and writes
- literals
- direct calls
- branch expressions
- blocks with ordered instructions and a result

A `let` expression is a good example:

```gleam
let y = x
y
```

The AST stores this as a `let` binding followed by a variable expression. The IR
allocates a local for `y`, writes the lowered value of `x` into it, and reads
`y` as the block result.

```text
local y: Int
set y, get x
result get y
```

## Evaluation order

Source syntax can leave some details implicit. IR should make them explicit.
In a block, expressions are evaluated from top to bottom:

```gleam
fn main() {
  let one = 1
  let two = one
  two
}
```

The IR stores the two local writes in order, then stores the block result. This
is helpful for WebAssembly because WebAssembly code generation also needs a clear
order of instructions.

## Locals

Names are useful for people, but compilers often prefer stable IDs. During
lowering, parameters and local bindings become locals:

```text
LocalId(0): x: Int
LocalId(1): y: Int
```

A variable expression such as `y` becomes `LocalGet(LocalId(1))`. From this
point on, the compiler does not need to search scopes to understand which `y` is
being used. That work has already been reflected in the local allocation.

## Calls

A direct function call in Gleam:

```gleam
id(1)
```

lowers to an IR call with a function name and lowered arguments:

```rust
Call {
  function: "id",
  arguments: [1]
}
```

The type checker has already checked arity and argument types. Lowering can
therefore focus on representation: what is called, what values are passed, and
what type comes back.

## Branches

A simple `case` expression lowers into a branch expression:

```gleam
case x {
  0 -> 1
  _ -> 2
}
```

The IR stores the lowered subject, the patterns, and each branch body. This is
still higher-level than raw WebAssembly branching, but it is lower-level than
Gleam syntax. A later code generator can choose how to turn the branch into WASM
blocks, comparisons, and jumps.

## What this compiler lowers today

The current core IR handles:

- modules and functions
- function parameters
- local allocation for parameters and `let` bindings
- typed literals
- local reads and writes
- direct function calls
- blocks with ordered instructions and a result
- simple `case` expressions as branch expressions
- source spans on IR nodes where useful

The output is deterministic, which makes it suitable for snapshot-style tests and
for reading while the compiler grows.

[^1]: Wikipedia, "Intermediate representation": https://en.wikipedia.org/wiki/Intermediate_representation

[^2]: LLVM, "LLVM Bitcode File Format": https://llvm.org/docs/BitCodeFormat.html

[^3]: Ray Toal, "Intermediate Representations": https://cs.lmu.edu/~ray/notes/ir/

[^4]: Matt Warren, "Lowering in the C# Compiler": https://mattwarren.org/2017/05/25/Lowering-in-the-C-Compiler/
