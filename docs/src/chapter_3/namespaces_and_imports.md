# Namespaces and imports

Some languages keep all declarations in one namespace. Many languages do not.
A namespace is a logical group of names, so the same spelling can refer to
different kinds of declarations without colliding.

Rust makes this distinction explicit. Its reference defines separate namespaces
and explains that a use of a name searches the namespace appropriate for the
context.[^1] A field access, a type annotation, and a function call ask different
questions.

For a Gleam compiler, the useful namespaces include:

- values, such as functions, parameters, and local variables
- types, such as custom types and type aliases
- constructors, such as `Ok`, `Error`, or a user-defined record constructor
- fields, such as a labelled record field
- modules, such as `io` after `import gleam/io`

Separate namespaces let the resolver preserve the programmer's intent. In a type
annotation, `Result` should resolve as a type. In an expression, `Ok` should
resolve as a constructor. In `io.println`, `io` should resolve as a module name
before the compiler treats `println` as a module member.

## Qualified module names

Imports add module names to the module scope:

```gleam
import gleam/io
import gleam/string as text
```

Gleam module names come from file paths, and after importing a module the last
part of the module name is used locally. The `as` keyword gives a module a
different local name.[^2]

```gleam
io.println("Hello")
text.reverse("abc")
```

The parser sees `io.println` and `text.reverse` as field access syntax. It
cannot know from syntax alone whether the left side is a module or a value. Name
resolution checks the left side. If it is an imported module, the compiler
records the expression as a qualified module member.

## Unqualified imports

Gleam also supports unqualified imports:

```gleam
import gleam/io.{println}

pub fn main() {
  println("Hello")
}
```

Here `println` can be used without writing `io.`. The Gleam tour recommends
qualified imports for functions because they make the defining module clear, but
unqualified imports are available when they improve readability.[^3]

Types can be imported unqualified too:

```gleam
import gleam/string_tree.{type StringTree}
```

The `type` marker matters. It tells the resolver that the imported name belongs
in the type namespace. Gleam code commonly imports types this way and calls
functions through their module name.[^4]

## Prelude names

Some names are available without an explicit import. These are prelude or
built-in names from the language and standard environment. In this compiler, the
resolver defines core type names such as `Int`, `Float`, `String`, `Bool`, `Nil`,
`List`, and `Result` in the module scope before it processes user declarations.

Prelude names are symbols too. Treating them that way keeps the rest of the
compiler uniform: a type annotation that mentions `Int` can point at a symbol
just like an annotation that mentions an imported type.

## Ambiguity

Unqualified imports can introduce ambiguity:

```gleam
import one.{id}
import two.{id}

fn main() {
  id(1)
}
```

If both imports introduce a value named `id`, the resolver should reject the
program instead of guessing. Qualified names keep the module path in the program
text; unqualified names move that distinction into the resolver's symbol table.

[^1]: Rust Reference, "Namespaces":
    https://doc.rust-lang.org/reference/names/namespaces.html
[^2]: Gleam Language Tour, "Modules":
    https://tour.gleam.run/basics/modules/
[^3]: Gleam Language Tour, "Unqualified imports":
    https://tour.gleam.run/basics/unqualified-imports/
[^4]: Gleam Language Tour, "Type imports":
    https://tour.gleam.run/basics/type-imports/
