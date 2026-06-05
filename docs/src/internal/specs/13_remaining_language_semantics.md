# Open language semantics

Syntax, resolution, type checking, pattern matching, and IR lowering are
documented in the development notes. This spec tracks represented forms that
still need full Gleam semantics rather than placeholder or partial behavior.

## Target groups

Target groups should select declarations by the requested compile target. The
compiler must define when non-selected declarations are ignored, how selected
declarations affect interfaces, and how diagnostics report target-specific
unsupported code.

## Bit-string matching

Bit-string deconstruction should implement segment matching, binding, size
checks, unit checks, signed/unsigned behavior, and endian behavior. Invalid
segment access must fail before unsafe memory reads.

## Closures and captures

Closure values should invoke captured environments correctly. Captures may
include managed and scalar values. The runtime and backend must define how
scalar captures are boxed or stored and how indirect calls recover captured
values.

## `use` lowering

`use` should lower to the same callback-passing semantics as Gleam. Evaluation
order, callback parameters, captures, and failure paths must be explicit in IR
before code generation.

## Record updates

Record update should allocate or construct a new record/custom value with
updated fields while preserving unchanged fields in declaration order. It must
work for managed and scalar fields.

## Active tasks

See [Language semantics tasks](../tasks/13_remaining_language_semantics.md).
