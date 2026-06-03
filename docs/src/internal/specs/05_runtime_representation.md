# Runtime representation

WebAssembly provides numbers, functions, tables, and linear memory. Gleam values
such as strings, lists, tuples, records, closures, and custom types use explicit
runtime representations in linear memory.

## Object model

Heap objects begin with an 8-byte header:

```text
0..4  tag:  i32
4..8  size: i32, length, or arity depending on object kind
8..   payload
```

Objects are aligned to 8 bytes. Managed values cross the WASM boundary as `i32`
pointers into linear memory.

## Tags

| Tag | Object kind  |
| --- | ------------ |
| 1   | String       |
| 2   | List cons    |
| 3   | Tuple        |
| 4   | Record       |
| 5   | Custom value |
| 6   | Closure      |

The empty list can be represented as the null pointer.

## Layouts

Strings store a byte length in the header's second word and UTF-8 bytes in the
payload, padded to alignment.

Lists use cons cells with a head value and tail pointer. Tuples and records use
fixed field arrays. Custom values add a constructor tag before payload fields.
Closures store a function/table identifier plus captured values.

## Allocation and ABI

The current runtime uses a bump allocator with a mutable heap pointer and no
freeing. This is sufficient for layout tests and small examples. Scalars keep
using direct WASM values; managed values use `i32` pointers.

## Tests

Runtime tests should inspect memory in Wasmtime and check tags, lengths, fields,
and payload bytes directly.
