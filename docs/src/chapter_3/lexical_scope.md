# Lexical scope and shadowing

A scope is the region of code where a name can be used. In a lexically scoped
language, that region is determined by the source text. A compiler can decide
which binding a name refers to by looking at the program structure rather than by
running the program.[^1]

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
`x` inside that block refers to the local binding. Outside the block, `x` means
the parameter.

Name lookup follows a chain. When resolving a reference, the compiler checks the
nearest scope first. If the name is not there, it checks the parent scope, and so
on until it reaches the module scope.

```text
nested block scope
  -> function scope
  -> module scope
```

This is sometimes called the most closely nested rule: the closest declaration
that is visible from the reference is the one that wins.[^2]

## Lexical scope vs dynamic scope

Dynamic scope uses the call stack instead of the source tree. Under dynamic
scope, a name can resolve differently depending on which functions were called
to reach the current code.[^3]

```text
lexical scope:  look around the source location
dynamic scope:  look through the active calls at runtime
```

Most modern statically typed languages, including Gleam and Rust, use lexical
scope. This makes name resolution a compile-time analysis. The compiler can
store the result once, produce better diagnostics, and let later passes work
with symbol IDs instead of repeatedly interpreting textual names.

Dynamic scope is useful as a contrast. It shows that name resolution is governed
by the language definition, which decides where the search is allowed to go.

## Shadowing

Shadowing allows a more local binding to use the same spelling as an outer
binding:

```gleam
fn describe(name) {
  let name = "Hello, " <> name
  name
}
```

Gleam permits variable names to be reused by later `let` bindings, while the
values themselves remain immutable.[^4] This helps when a value is refined step
by step. The compiler treats each binding as a distinct symbol, even if they
share the same text.

Shadowing is different from a duplicate definition in the same scope:

```gleam
fn main(x, x) {
  x
}
```

There are two parameters named `x` in one function scope. A reference to `x` in
the body would not have one clear target, so the compiler reports a duplicate
name error.

[^1]: University of Washington CSE 341, "Lexical and Dynamic Scoping":
    https://courses.cs.washington.edu/courses/cse341/15au/general-concepts/scoping.html
[^2]: NYU Compilers Lecture 3:
    https://cs.nyu.edu/~gottlieb/courses/compilers/lectures/lecture-03.html
[^3]: Carnegie Mellon 17-363, "Names, Scopes, and Bindings":
    https://www.cs.cmu.edu/~aldrich/courses/17-363-fa21/slides/lecture3-binding.pdf
[^4]: Gleam Language Tour, "Assignments":
    https://tour.gleam.run/basics/assignments/
