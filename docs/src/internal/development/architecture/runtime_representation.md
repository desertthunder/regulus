# Runtime representation

WebAssembly provides numbers, functions, tables, and linear memory. Gleam values
such as strings, bit arrays, lists, tuples, records, closures, and custom types
use explicit runtime representations in linear memory.

## Scalar values

Scalars stay unboxed inside WebAssembly functions and locals:

| Gleam value | WASM value                                          |
| ----------- | --------------------------------------------------- |
| `Int`       | `i64`                                               |
| `Float`     | `f64`                                               |
| `Bool`      | `i32`, where `0` is false and `1` is true           |
| `Nil`       | no result value; stored fields use `0`              |
| `Order`     | `i32`, where `-1`, `0`, and `1` mean LT, EQ, and GT |

Managed values cross internal and host ABI boundaries as `i32` pointers into
linear memory. The null pointer is reserved for the empty list.

## Object model

Heap objects begin with an 8-byte header:

```text
0..4  tag:  i32
4..8  size: i32, length, arity, or field count depending on object kind
8..   payload
```

Objects are aligned to 8 bytes. Payload fields that can contain arbitrary Gleam
values use 8-byte slots. Scalar `Int` and `Float` values occupy the whole slot.
Pointer, bool, nil, and ordering values use the low 32 bits and leave padding
zeroed.

The current runtime uses a bump allocator with a mutable heap pointer and no
freeing. Allocation checks the aligned end pointer and grows linear memory when
needed. Objects are non-moving: existing managed pointers remain stable across
`memory.grow` and live until the instance is reset. Static objects are emitted
as data segments before the dynamic heap.

## Tags

| Tag | Object kind   | Header word 1 | Payload                                           |
| --- | ------------- | ------------- | ------------------------------------------------- |
| 1   | String        | byte length   | UTF-8 bytes, padded                               |
| 2   | List cons     | arity `2`     | head slot, tail pointer                           |
| 3   | Tuple         | arity         | field slots                                       |
| 4   | Record        | field count   | field slots in declaration order                  |
| 5   | Custom value  | field count   | constructor tag, field slots                      |
| 6   | Closure       | capture count | function id, capture pointers                     |
| 7   | Bit array     | bit length    | packed bits, most-significant bit first           |
| 8   | Opaque        | `0`           | dependency type tag, host/runtime payload pointer |
| 9   | Runtime error | field count   | reason tag, field slots                           |
| 10  | Panic value   | field count   | reason tag, field slots                           |

## Managed value rules

Strings store UTF-8 bytes exactly as written after unescaping. String equality
and ordering compare UTF-8 bytes after length checks.

Bit arrays store a bit length and packed payload bytes. Segment operations must
preserve bit offsets and reject invalid sizes before memory access.

Lists use cons cells. The empty list is `0`; non-empty lists are tag-2 objects.
The tail field is either another cons pointer or `0`.

Tuples and records use the same field-slot layout. Records keep fields in the
constructor declaration order so field access can use a fixed offset.

Custom values represent user-defined variants, plus dependency-defined `Result`
and `Option` values. `Ok`, `Error`, `Some`, and `None` are constructor tags in
normal tag-5 custom objects. Runtime error objects use tag 9 and are separate
from the `Result.Error` constructor.

Closures store a stable function id followed by captured values in 8-byte
slots. Managed captures use pointers and scalar captures use their raw slot
representation.

Opaque values are owned by the dependency or host interface that created them.
The backend may pass opaque pointers around and compare identity, but may not
inspect payload contents without a dependency-specific helper.

Panic and runtime error objects are materialized when a helper needs to report
or render a failure. Direct panic paths may trap without allocating one.
Allocation failure writes a reserved tag-10 panic object before trapping. Its
reason tag is `1`, payload slot 0 is the requested allocation size, and payload
slot 1 is the heap pointer before allocation.

## ABI and ownership

Internal calls pass scalars as raw WASM values and managed values as pointers.
Host ABI support is target-specific. For Wasmtime, managed pointers are borrowed
by the host; ownership stays with the guest runtime unless an adapter documents
transfer. A borrowed pointer may be read after the exporting call returns and
remains stable until the Wasm instance is reset or a future explicit arena reset
runs. Hosts must not retain pointers across those reset boundaries.

## Tests

Runtime tests inspect encoded objects directly. Wasmtime tests inspect static
and dynamic memory for tags, lengths, fields, payload bytes, and alignment.
