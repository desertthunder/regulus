# Closures and intrinsic callbacks

Regulus uses one closure ABI everywhere. Ordinary functions, compiler
intrinsics, runtime helpers, stdlib adapters, and future host adapters must not
invent separate callback conventions.

## Closure value ABI

A closure value is a managed `i32` pointer to a runtime closure object. The
object stores:

- the runtime object tag for closures
- the capture count
- the compiler-assigned function id
- capture slots in the same order used by lowering

Callback arguments and return values use the ordinary call ABI:

- `Int` uses `i64`
- `Float` uses `f64`
- `Bool` uses `i32`
- managed values use `i32` pointers
- `Nil` has no result

Captured values keep their normal representation in closure capture slots. Code
that invokes a closure must load captures from the closure object, append the
explicit callback arguments, and dispatch to the target function by using the
same function id checks as ordinary indirect calls.

## Intrinsic callback lowering

Plain runtime WAT helpers must not call user closures directly. A helper has no
independent authority to interpret closure objects or choose a function target.

Any intrinsic that invokes a user function must use one of these forms:

1. Lower to IR that contains ordinary indirect calls.
2. Lower to a compiler-generated adapter that is equivalent to IR indirect-call
   lowering.

Runtime helpers may still provide low-level operations such as allocation,
property lookup, list traversal, dynamic classification, and error allocation.
Closure invocation belongs to compiler-owned lowering.

## Reuse requirements

Compiler-generated adapters must reuse the ordinary closure machinery:

- same closure object layout
- same capture slot offsets
- same function id assignment
- same indirect-call dispatch checks
- same argument and result ABI
- same diagnostics for unsupported parameter or result shapes

If an intrinsic callback cannot use that machinery, lowering must reject the
program before WAT assembly with a source-spanned diagnostic.

## Callback-taking stdlib members

The following stdlib functions should be implemented through the shared
callback lowering mechanism:

| Module | Members |
| ------ | ------- |
| `gleam/list` | `map`, `fold` |
| `gleam/result` | `map` |
| `gleam/option` | `map` |
| `gleam/function` | `compose`, `flip` |
| `gleam/dynamic/decode` | `field`, `map`, `then`, `recursive` |

These functions should lower to IR or generated adapters, not bespoke WAT
callback code. The adapter owns loop or branching structure, while any runtime
helper only handles data access.
