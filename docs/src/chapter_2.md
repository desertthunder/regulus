# Name resolution

Names are how a program connects pieces of code. In Gleam, a name might refer to
a function, a parameter, a local variable, or an imported module:

```gleam
import gleam/io

fn greet(name) {
  let message = "Hello, " <> name
  io.println(message)
}
```

This program has several names: `io`, `greet`, `name`, `message`, and `println`.
The source text only tells us how those names are spelled. Name resolution works
out what each name refers to.

A common definition is that name resolution associates identifiers in a program
with the declarations they denote.[^1] That sounds small, but it is one of the
places where a language's rules become concrete. The compiler has to decide
which names are visible, what happens when two names are the same, and whether a
name like `io.println` means a module function or a field access.

## Bindings and references

A binding introduces a name. A reference uses a name.

In this function, `name` and `message` are bindings:

```gleam
fn greet(name) {
  let message = name
  message
}
```

The final `message` is a reference. It points back to the `let message = ...`
binding. The `name` on the right-hand side of the `let` points back to the
function parameter.

After name resolution, the compiler does not need to ask, "Which `message` is
this?" The reference has been connected to a stable symbol. A symbol is the
compiler's record for a binding: its name, where it was defined, what kind of
binding it is, and a small ID that other data structures can store.

## Scopes

A scope is a region of code where a set of names can be used. A module has a
scope for top-level names, such as imports and functions. A function body has a
scope for parameters and local variables. A nested block can have its own scope.

```gleam
fn main(x) {
  let y = x
  {
    let x = 1
    x
  }
  y
}
```

The inner `let x = 1` shadows the parameter `x` inside the nested block. The
`x` inside that block refers to the local binding. Outside the block, `x` still
means the parameter.

A useful way to picture this is as a chain. When resolving a reference, the
compiler checks the nearest scope first. If the name is not there, it checks the
parent scope, and so on until it reaches the module scope.

```text
nested block scope
  -> function scope
  -> module scope
```

This nearest-scope rule is an example of specificity: the most specific piece of
program structure wins over a more general one. Will Crichton describes
specificity as a recurring idea in programming language design, where more local
or more precise information takes priority over broader defaults.[^2]

## Duplicate names

Name resolution also rejects some programs. Two bindings with the same name in
the same scope are ambiguous:

```gleam
fn main(x, x) {
  x
}
```

There are two parameters named `x`. A reference to `x` in the function body would
not have one clear target, so the compiler reports a duplicate-name error.

Shadowing is different. This is allowed by the current resolver because the two
`x` bindings live in different scopes:

```gleam
fn main(x) {
  {
    let x = 1
    x
  }
  x
}
```

The inner `x` is more specific while the compiler is inside the nested block.
The outer `x` is still available after the block ends.

## Unknown names

If a reference cannot be found in any visible scope, the compiler reports an
unknown-name error:

```gleam
fn main() {
  missing
}
```

There is no parameter, local variable, imported module, or top-level function
named `missing`, so the reference cannot be connected to a symbol.

The error message should point at the reference, not just the whole function.
That is why syntax tree nodes keep source spans.

## Imports and qualified names

Imports add names to the module scope:

```gleam
import gleam/io
import gleam/list as list
```

The first import introduces the name `io`, taken from the final segment of
`gleam/io`. The second introduces the explicit alias `list`.

A call such as this has an interesting syntax-tree shape:

```gleam
io.println("hello")
```

The parser sees `io.println` as field access. It cannot know from syntax alone
whether `io` is a module or a value with a field named `println`. Name resolution
checks the name `io`. If `io` is an imported module, the compiler records the
reference as a qualified module member.

This is similar to the kind of separation described in the Rust compiler guide:
parsing builds syntax, and name resolution works out what paths and identifiers
mean in their context.[^3]

## What this compiler resolves today

The current resolver handles:

- top-level function names
- imported module names and aliases
- function parameters
- local `let` bindings
- variable references
- nested block shadowing
- simple `case` clause pattern bindings
- qualified module references such as `io.println`

It reports duplicate names in the same scope and unknown names at their source
location.

[^1]: Wikipedia, "Name resolution (programming languages)": https://en.wikipedia.org/wiki/Name_resolution_(programming_languages)

[^2]: Will Crichton, "Specificity in Programming Languages": https://willcrichton.net/notes/specificity-programming-languages/

[^3]: Rust Compiler Development Guide, "Name resolution": https://rustc-dev-guide.rust-lang.org/name-resolution.html

<!--
TODO (research):
  - Lexical scope vs dynamic scope
  - Shadowing rules
  - Separate namespaces in languages
      - value names
      - type names
      - constructors
      - fields
      - modules
  - Qualified vs unqualified imports
  - Prelude/default imports
  - Module privacy and public/private declarations
  - Pattern bindings and how patterns introduce names
  - Ambiguous imports and duplicate definitions
  - Cross-module name resolution in package graphs
  - How Gleam specifically handles imports, aliases, custom type constructors, and record fields
  - How Rust’s name resolution uses namespaces, modules, and visibility as a comparison point
-->
