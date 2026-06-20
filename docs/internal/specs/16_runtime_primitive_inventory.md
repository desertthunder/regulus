# Runtime Primitive Inventory

This inventory tracks every current `runtime::stdlib_runtime_primitive` entry.
It is a deletion plan, not a justification for keeping library behavior in the
compiler.

Use these owner values:

- `runtime primitive`: keep only if it is representation, allocation, ABI,
  host/debug, or low-level data access.
- `library source`: delete after an upstream `gleam_stdlib` source proof covers
  the function.
- `package asset`: replace with a validated stdlib package asset or a narrow
  native-handle primitive.

## Runtime Primitive

These entries are acceptable compiler/runtime responsibilities, though some
should eventually be renamed away from stdlib module/member names.

| Entry                            | Owner             | Current blocker                              | Deletion or move condition                                                         |
| -------------------------------- | ----------------- | -------------------------------------------- | ---------------------------------------------------------------------------------- |
| `gleam/bit_array.append`         | runtime primitive | Bit-array storage helper.                    | Keep as bit-array primitive or expose through compiled source calling a primitive. |
| `gleam/bit_array.bit_size`       | runtime primitive | Bit-array layout metadata.                   | Keep as bit-array primitive or expose through compiled source calling a primitive. |
| `gleam/bit_array.byte_size`      | runtime primitive | Bit-array layout metadata.                   | Keep as bit-array primitive or expose through compiled source calling a primitive. |
| `gleam/bit_array.concat`         | runtime primitive | Bit-array storage helper.                    | Keep as bit-array primitive or expose through compiled source calling a primitive. |
| `gleam/bit_array.is_empty`       | runtime primitive | Bit-array layout metadata.                   | Keep as bit-array primitive or expose through compiled source calling a primitive. |
| `gleam/bit_array.starts_with`    | runtime primitive | Bit-array matching helper.                   | Keep as bit-array primitive or expose through compiled source calling a primitive. |
| `gleam/dynamic.array`            | runtime primitive | Dynamic representation.                      | Keep as dynamic construction primitive.                                            |
| `gleam/dynamic.bit_array`        | runtime primitive | Dynamic representation.                      | Keep as dynamic construction primitive.                                            |
| `gleam/dynamic.bool`             | runtime primitive | Dynamic representation.                      | Keep as dynamic construction primitive.                                            |
| `gleam/dynamic.classify`         | runtime primitive | Dynamic representation inspection.           | Keep as dynamic inspection primitive.                                              |
| `gleam/dynamic.float`            | runtime primitive | Dynamic representation.                      | Keep as dynamic construction primitive.                                            |
| `gleam/dynamic.int`              | runtime primitive | Dynamic representation.                      | Keep as dynamic construction primitive.                                            |
| `gleam/dynamic.list`             | runtime primitive | Dynamic representation.                      | Keep as dynamic construction primitive.                                            |
| `gleam/dynamic.nil`              | runtime primitive | Dynamic representation.                      | Keep as dynamic construction primitive.                                            |
| `gleam/dynamic.properties`       | runtime primitive | Dynamic representation.                      | Keep as dynamic construction primitive.                                            |
| `gleam/dynamic.string`           | runtime primitive | Dynamic representation.                      | Keep as dynamic construction primitive.                                            |
| `gleam/dynamic/decode.bit_array` | runtime primitive | Decoder primitive value.                     | Keep as decoder primitive or replace with upstream source wrapping a primitive.    |
| `gleam/dynamic/decode.bool`      | runtime primitive | Decoder primitive value.                     | Keep as decoder primitive or replace with upstream source wrapping a primitive.    |
| `gleam/dynamic/decode.dynamic`   | runtime primitive | Decoder primitive value.                     | Keep as decoder primitive or replace with upstream source wrapping a primitive.    |
| `gleam/dynamic/decode.float`     | runtime primitive | Decoder primitive value.                     | Keep as decoder primitive or replace with upstream source wrapping a primitive.    |
| `gleam/dynamic/decode.int`       | runtime primitive | Decoder primitive value.                     | Keep as decoder primitive or replace with upstream source wrapping a primitive.    |
| `gleam/dynamic/decode.list`      | runtime primitive | Decoder primitive value with nested decoder. | Keep only primitive representation; move combinator behavior to source.            |
| `gleam/dynamic/decode.optional`  | runtime primitive | Decoder primitive value with nested decoder. | Keep only primitive representation; move combinator behavior to source.            |
| `gleam/dynamic/decode.run`       | runtime primitive | Dynamic decode execution.                    | Keep as dynamic boundary primitive.                                                |
| `gleam/dynamic/decode.string`    | runtime primitive | Decoder primitive value.                     | Keep as decoder primitive or replace with upstream source wrapping a primitive.    |
| `gleam/io.debug`                 | runtime primitive | Debug rendering of managed values.           | Keep as debug primitive, renamed away from stdlib dispatch.                        |
| `gleam/string.append`            | runtime primitive | String storage allocation.                   | Keep as string primitive or expose through compiled source calling a primitive.    |
| `gleam/string.concat`            | runtime primitive | String list concatenation allocation.        | Keep as string primitive or expose through compiled source calling a primitive.    |
| `gleam/string.is_empty`          | runtime primitive | String layout metadata.                      | Keep as string primitive or expose through compiled source calling a primitive.    |
| `gleam/string.length`            | runtime primitive | String layout metadata.                      | Keep as string primitive or expose through compiled source calling a primitive.    |

## Package Asset Or Native Handle

These entries should not remain as collection logic in the compiler. Decide
whether upstream stdlib should use a validated package asset such as `dict.mjs`
or a narrow native-handle primitive.

| Entry                 | Owner         | Current blocker                                         | Deletion or move condition                                                |
| --------------------- | ------------- | ------------------------------------------------------- | ------------------------------------------------------------------------- |
| `gleam/dict.delete`   | package asset | Dict package asset/native representation not validated. | Replace with compiled upstream source plus validated asset/native handle. |
| `gleam/dict.get`      | package asset | Dict package asset/native representation not validated. | Replace with compiled upstream source plus validated asset/native handle. |
| `gleam/dict.has_key`  | package asset | Dict package asset/native representation not validated. | Replace with compiled upstream source plus validated asset/native handle. |
| `gleam/dict.insert`   | package asset | Dict package asset/native representation not validated. | Replace with compiled upstream source plus validated asset/native handle. |
| `gleam/dict.is_empty` | package asset | Dict package asset/native representation not validated. | Replace with compiled upstream source plus validated asset/native handle. |
| `gleam/dict.new`      | package asset | Dict package asset/native representation not validated. | Replace with compiled upstream source plus validated asset/native handle. |
| `gleam/dict.size`     | package asset | Dict package asset/native representation not validated. | Replace with compiled upstream source plus validated asset/native handle. |

## Library Source

These entries are ordinary library behavior. Delete each runtime dispatch arm
after a source proof shows the upstream function compiles and links from
`gleam_stdlib`.

| Entry                     | Owner          | Current blocker                                                    | Deletion condition                                                                         |
| ------------------------- | -------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------ |
| `gleam/bool.compare`      | library source | Not present in the current upstream fixture.                       | Keep runtime dispatch until upstream source or a replacement proof exists.                 |
| `gleam/bool.negate`       | library source | Pure source proof exists; full module now lowers.                  | Runtime table and direct codegen dispatch deleted.                                         |
| `gleam/bool.to_string`    | library source | Source-backed link proof exists; registry path still needs it.     | Delete after stdlib source is loaded by default.                                           |
| `gleam/float.compare`     | library source | Source-backed link proof exists with `gleam/order`.                | Delete after stdlib source is loaded by default.                                           |
| `gleam/float.max`         | library source | Source-backed link proof exists; registry path still needs it.     | Delete after stdlib source is loaded by default.                                           |
| `gleam/float.min`         | library source | Source-backed link proof exists; registry path still needs it.     | Delete after stdlib source is loaded by default.                                           |
| `gleam/float.negate`      | library source | Source-backed link proof exists; registry path still needs it.     | Delete after stdlib source is loaded by default.                                           |
| `gleam/float.to_string`   | library source | Full module needs imported dependencies or native externals.       | Compile upstream function or isolate required primitive, then delete runtime dispatch.     |
| `gleam/function.compose`  | library source | Not present in the current upstream fixture.                       | Keep runtime dispatch until upstream source or a replacement proof exists.                 |
| `gleam/function.constant` | library source | Not present in the current upstream fixture.                       | Keep runtime dispatch until upstream source or a replacement proof exists.                 |
| `gleam/function.flip`     | library source | Not present in the current upstream fixture.                       | Keep runtime dispatch until upstream source or a replacement proof exists.                 |
| `gleam/function.identity` | library source | Source-backed link proof exists; registry path still needs it.     | Delete after stdlib source is loaded by default.                                           |
| `gleam/int.to_string`     | library source | Full module needs imported `gleam/float` source or native support. | Compile upstream function or isolate required primitive, then delete runtime dispatch.     |
| `gleam/list.fold`         | library source | Source-backed link proof and registry behavior fixture exist.      | Runtime table dispatch deleted; registry-backed lowering adapter still remains.            |
| `gleam/list.length`       | library source | Source-backed link proof exists; registry path still needs it.     | Delete after stdlib source is loaded by default.                                           |
| `gleam/list.map`          | library source | Source-backed link proof and registry behavior fixture exist.      | Runtime table dispatch deleted; registry-backed lowering adapter still remains.            |
| `gleam/list.reverse`      | library source | Source-backed link proof exists; registry path still needs it.     | Delete after stdlib source is loaded by default.                                           |
| `gleam/option.map`        | library source | Pure source proof exists for selected functions.                   | Runtime table dispatch deleted; registry-backed lowering adapter still remains.            |
| `gleam/result.map`        | library source | Pure source proof exists for selected functions.                   | Runtime table dispatch deleted; registry-backed lowering adapter still remains.            |

## First Deletion Slice

Completed entries from the first source-backed deletion slice:

1. `gleam/bool.negate`
2. `gleam/list.fold`
3. `gleam/list.map`
4. `gleam/option.map`
5. `gleam/result.map`

Remaining deletion work requires:

- source proof compiles the upstream function from `gleam_stdlib`
- behavior fixture still passes
- linked debug dump contains `gleam_stdlib:gleam/<module>.<function>`
- linked debug dump does not contain `__stdlib_gleam_<module>_<function>`
