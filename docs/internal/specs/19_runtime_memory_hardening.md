# Runtime memory hardening

The current runtime uses static data plus bump allocation in guest linear
memory. This is enough for tests and small examples, but it does not yet define
failure behavior, reclamation, or long-running host interaction.

## Scope

This spec covers the next runtime-memory milestone. It does not require garbage
collection before examples can run, but it should make allocation behavior and
host ownership explicit.

## Current model

- Static managed objects are emitted as data segments.
- Dynamic allocation starts after static data.
- `__alloc` aligns requests to 8 bytes.
- Allocation advances the heap pointer.
- Objects are non-moving.
- The runtime does not free individual objects.

## Required hardening

The runtime should define and test:

- allocation size overflow checks
- heap-limit checks
- `memory.grow` behavior where growth is allowed
- deterministic allocation failure paths
- object header validation for host readers
- clear ownership rules for host-provided managed pointers
- diagnostics or traps for invalid runtime helper use

## Deferred memory strategies

Freeing, garbage collection, reference counting, arena resets, and moving
compaction are deferred. Before any of them are implemented, the host ABI must
say whether pointers can survive calls, instance resets, or arena resets.

## Host interaction

Hosts may inspect guest-managed values through exported helpers. Hosts must not
mutate runtime object memory. Future APIs that accept host-provided handles or
managed pointers must document ownership and lifetime rules.

## Runtime helper inventory

Allocation helpers:

- `__alloc`, `__allocation_fail`, and `__last_panic`

Managed value helpers:

- tuple, record, custom, closure, opaque, option, order, error, and panic
  constructors
- raw field readers used by generated code

Closure helpers:

- closure allocation and indirect-call capture layout helpers

Equality and ordering helpers:

- structural equality and comparison for strings, bit arrays, lists, tuples,
  records, custom values, and scalar slots

Debug helpers:

- debug tags, panic/error reasons, and payload readers

Dynamic value helpers:

- dynamic value constructors, classifiers, field readers, decoder
  constructors, and decoder runners

Host adapter helpers:

- JS adapter exports for allocation, string creation/reading, managed value
  tags, arity, constructors, fields, and opaque handle readers

## Host reader validation

Exported JS adapter reader helpers validate object headers before reading:

- `__regulus_value_tag(0)` returns `0` as the nil-list/null sentinel.
- Non-zero reader pointers must reference a known runtime object tag.
- String readers require tag `1` and validate the byte range.
- Handle readers require tag `8` and validate the opaque payload range.
- `__regulus_value_arity` validates the object range. It returns field counts
  for field objects and `0` for strings and bit arrays.
- `__regulus_value_constructor` validates the object range. It returns the
  constructor or reason tag for custom, error, and panic objects, and `0` for
  other valid objects.
- `__regulus_value_field` only accepts list cons, tuple, record, custom, error,
  and panic objects. It traps when the field index is out of range.

Malformed non-zero pointers, unknown tags, wrong reader/object pairs, oversized
payloads, and out-of-range field indexes trap. The only sentinel returns in the
reader surface are nil tag `0`, non-constructor value constructor `0`, and
non-field byte-object arity `0`.

## Active tasks

See [Runtime memory hardening tasks](../tasks/20_runtime_memory_hardening.md).
