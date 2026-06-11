# Type checking and type inference

A type checker proves that the program's values are used consistently before
lowering begins. In Gleam, integer arithmetic must receive integers, string
concatenation must receive strings, function calls must match their parameter
types, and all branches of a `case` expression must agree on one result type.

Type inference fills in types that the programmer did not write. Gleam permits
many local annotations to be omitted, but function signatures and exported
module interfaces still need enough type information to make modules checkable
in isolation.[^annotations]

For this compiler, type checking has three jobs:

- assign a concrete `Type` to every supported expression
- reject mismatches with source-spanned diagnostics
- build module interfaces for imported values, constructors, and later phases

## A small example

This function has an annotated parameter and an inferred local:

```gleam
fn add_one(x: Int) -> Int {
  let one = 1
  x + one
}
```

The checker reads the parameter annotation first. That puts `x: Int` into the
function scope. The literal `1` has type `Int`, so the binding `one` also has
type `Int`. The `+` operator requires both operands to be `Int` and returns
`Int`, which matches the function's return annotation.

If the body changes to:

```gleam
x + "one"
```

the checker reports a mismatch at the string expression. Name resolution can
find the expression. Type checking decides whether using it there is legal.

## What Regulus checks today

Regulus uses an annotation-led checker. Function parameters must have type
annotations. Function return annotations are checked when present and inferred
from the body when absent. Local `let` bindings, block results, tuple values,
list values, records, constructor calls, function calls, operators, pipelines,
captures, anonymous functions, and `case` expressions are checked by walking the
expression tree.

The local type representation currently includes:

- scalar types: `Int`, `Float`, `String`, `BitArray`, `Bool`, and `Nil`
- compound types: tuples, lists, records, custom types, and functions
- generic type variables
- opaque custom types

Custom types are algebraic data types: constructors define the possible shapes
of a value, and pattern matching consumes that constructor information to prove
that each branch is well typed and, where supported, exhaustive.

The output is a `TypedModule`. It keeps the resolved module, typed top-level
functions, typed expressions by source span, and a `ModuleInterface` containing
functions, type declarations, and constructors. Lowering reads that typed output
instead of rechecking source syntax.

## Why interfaces matter

Modules are not checked as isolated text files forever. A downstream module may
call a public function, construct a public custom type, import a type name, or
refer to an opaque value without seeing the private implementation. Gleam's
package-interface format exposes public type definitions, type aliases,
constants, and functions, with type data attached to the public items.[^package]

Regulus uses the same idea locally. During project checking, it collects
function, constant, and constructor type information from modules and makes that
information available when another module imports or qualifies a name.

[^annotations]: Gleam Language Tour, "Type annotations": https://tour.gleam.run/basics/type-annotations/
[^package]: `gleam_package_interface`, "What's a package interface?": https://gleam-package-interface.hexdocs.pm/index.html
