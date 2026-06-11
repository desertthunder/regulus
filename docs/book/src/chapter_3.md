# Name resolution

Names are how a program connects pieces of code. In Gleam, a name might refer to
a function, a parameter, a local variable, a type, a constructor, a record field,
or an imported module:

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

A common compiler definition is that name resolution connects references to the
declarations they denote.[^1] That sounds small, but it is one of the places
where a language's rules become concrete. The compiler has to decide which names
are visible, what happens when two names are the same, and whether a name like
`io.println` means a module function or a field access.

For this project, the path is:

```text
Gleam AST
  -> scopes and symbols
  -> resolved references
```

The parser keeps names as text. The resolver turns those names into stable
symbol IDs and qualified member references that later compiler passes can use.

This chapter covers name resolution in more detail, including:

- lexical scope and shadowing
- bindings, references, scopes, and symbol tables
- namespaces, imports, and qualified names
- pattern bindings, visibility, and package graphs
- the resolver implemented in this compiler

[^1]: Pierre Neron, Andrew Tolmach, Eelco Visser, and Guido Wachsmuth, "A
    Theory of Name Resolution":
    https://researchr.org/publication/NeronTVW15
