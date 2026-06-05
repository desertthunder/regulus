# Type and generic inference

The inference layer provides the reusable machinery for inferring Gleam
expression, function, pattern, and module-interface types. It separates solver
state from source-level type names so later checker code can infer missing
annotations without confusing unknowns with named generic parameters.

## Inference variables

Inference variables are compiler-created placeholders for unknown types. Named
Gleam generics, such as `a` or `value`, remain source-level generic parameters.
This distinction lets the solver replace unknowns while preserving the public
shape of generic types.

A type scheme records a type plus the inference variables that were generalized.
Looking up a scheme instantiates those variables with fresh unknowns, so one
polymorphic value can be used at multiple concrete types.

## Constraints

Constraint generation walks expressions and patterns and records type equality
requirements. Literals produce known scalar types. Variables and constructors
come from the inference environment. Calls, pipelines, captures, anonymous
functions, records, lists, tuples, `case`, guards, and patterns add constraints
between inferred subexpressions and expected shapes.

The generator does not solve constraints while walking syntax. Keeping
collection separate from solving makes the inferred relationships easier to
inspect and gives diagnostics access to the original source span for each
constraint.

## Substitution and unification

Solving uses substitutions from inference variables to types. Walking a type
applies all known substitutions recursively through tuples, lists, records,
custom types, opaque types, functions, and fields.

Unification recursively compares type structures. Matching inference variables
are bound through substitutions. Occurs checks reject recursive bindings such as
an unknown that would have to equal `List(unknown)`.

## Generalization

Top-level definitions can generalize unsolved variables after their constraints
are solved. Local bindings generalize only variables that are not free in the
outer environment. This prevents a local polymorphic scheme from accidentally
capturing type information that belongs to an enclosing scope.

## Interfaces

Inference interfaces store public value and constructor schemes. Imported module
interfaces populate inference environments with qualified names, and each lookup
instantiates the imported scheme with fresh variables. Generic constructors use
the same scheme mechanism as generic functions and values.

## Diagnostics

Inference errors are represented at the solver and generator boundaries. The
constraint that triggers an error carries the source span, allowing later type
checking diagnostics to report incompatible, ambiguous, recursive, constructor,
field, branch, and pattern mismatches at the expression or pattern that caused
them.
