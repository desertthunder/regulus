# Runtime Primitive Inventory

This inventory tracks the stdlib-shaped runtime entry points by status. It is a
deletion plan, not a justification for keeping library behavior in the
compiler.

Read it by section:

- `Kept Runtime Primitives`: still valid compiler/runtime responsibilities.
- `Transitional Package Asset Or Native Handle`: still present, but only until
  package asset or native-handle support replaces collection logic in the
  compiler.
- `Removed Library Dispatch`: deleted from runtime dispatch and direct stdlib
  codegen paths. Do not re-add these as registry or runtime library behavior.
- `Next Removal Candidates`: live areas that should shrink after their blocker
  category is implemented.

## Kept Runtime Primitives

These entries are acceptable compiler/runtime responsibilities, though some
should eventually be renamed away from stdlib module/member names.

| Entry                            | Status            | Why it remains                               | Deletion or move condition                                                         |
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
| `gleam/dynamic/decode.at`        | runtime primitive | Decoder traversal over dynamic values.       | Move combinator behavior to source once upstream decode compiles.                  |
| `gleam/dynamic/decode.bit_array` | runtime primitive | Decoder primitive value.                     | Keep as decoder primitive or replace with upstream source wrapping a primitive.    |
| `gleam/dynamic/decode.bool`      | runtime primitive | Decoder primitive value.                     | Keep as decoder primitive or replace with upstream source wrapping a primitive.    |
| `gleam/dynamic/decode.dynamic`   | runtime primitive | Decoder primitive value.                     | Keep as decoder primitive or replace with upstream source wrapping a primitive.    |
| `gleam/dynamic/decode.field`     | runtime primitive | Decoder field traversal with closure dispatch. | Move combinator behavior to source once upstream decode compiles.                |
| `gleam/dynamic/decode.float`     | runtime primitive | Decoder primitive value.                     | Keep as decoder primitive or replace with upstream source wrapping a primitive.    |
| `gleam/dynamic/decode.int`       | runtime primitive | Decoder primitive value.                     | Keep as decoder primitive or replace with upstream source wrapping a primitive.    |
| `gleam/dynamic/decode.failure`   | runtime primitive | Decoder error primitive value.               | Keep only primitive representation; move combinator behavior to source.            |
| `gleam/dynamic/decode.list`      | runtime primitive | Decoder primitive value with nested decoder. | Keep only primitive representation; move combinator behavior to source.            |
| `gleam/dynamic/decode.map`       | runtime primitive | Decoder value using normal closure dispatch. | Move combinator behavior to source once upstream decode compiles.                  |
| `gleam/dynamic/decode.one_of`    | runtime primitive | Decoder fallback primitive value.            | Move combinator behavior to source once upstream decode compiles.                  |
| `gleam/dynamic/decode.optional`  | runtime primitive | Decoder primitive value with nested decoder. | Keep only primitive representation; move combinator behavior to source.            |
| `gleam/dynamic/decode.recursive` | runtime primitive | Decoder thunk using normal closure dispatch. | Move combinator behavior to source once upstream decode compiles.                  |
| `gleam/dynamic/decode.run`       | runtime primitive | Dynamic decode execution.                    | Keep as dynamic boundary primitive.                                                |
| `gleam/dynamic/decode.subfield`  | runtime primitive | Decoder nested field traversal.              | Move combinator behavior to source once upstream decode compiles.                  |
| `gleam/dynamic/decode.success`   | runtime primitive | Decoder success primitive value.             | Keep only primitive representation; move combinator behavior to source.            |
| `gleam/dynamic/decode.string`    | runtime primitive | Decoder primitive value.                     | Keep as decoder primitive or replace with upstream source wrapping a primitive.    |
| `gleam/dynamic/decode.then`      | runtime primitive | Decoder value using normal closure dispatch. | Move combinator behavior to source once upstream decode compiles.                  |
| `gleam/io.debug`                 | runtime primitive | Debug rendering of managed values.           | Keep as debug primitive, renamed away from stdlib dispatch.                        |
| `gleam/string.append`            | runtime primitive | String storage allocation.                   | Keep as string primitive or expose through compiled source calling a primitive.    |
| `gleam/string.concat`            | runtime primitive | String list concatenation allocation.        | Keep as string primitive or expose through compiled source calling a primitive.    |
| `gleam/string.is_empty`          | runtime primitive | String layout metadata.                      | Keep as string primitive or expose through compiled source calling a primitive.    |
| `gleam/string.length`            | runtime primitive | String layout metadata.                      | Keep as string primitive or expose through compiled source calling a primitive.    |

## Transitional Package Asset Or Native Handle

These entries should not remain as collection logic in the compiler. Upstream stdlib should use a
narrow native-handle primitive.

| Entry                 | Status        | Current blocker                                         | Removal condition                                                         |
| --------------------- | ------------- | ------------------------------------------------------- | ------------------------------------------------------------------------- |
| `gleam/dict.delete`   | package asset | Dict package asset/native representation not validated. | Replace with compiled upstream source plus validated asset/native handle. |
| `gleam/dict.get`      | package asset | Dict package asset/native representation not validated. | Replace with compiled upstream source plus validated asset/native handle. |
| `gleam/dict.has_key`  | package asset | Dict package asset/native representation not validated. | Replace with compiled upstream source plus validated asset/native handle. |
| `gleam/dict.insert`   | package asset | Dict package asset/native representation not validated. | Replace with compiled upstream source plus validated asset/native handle. |
| `gleam/dict.is_empty` | package asset | Dict package asset/native representation not validated. | Replace with compiled upstream source plus validated asset/native handle. |
| `gleam/dict.new`      | package asset | Dict package asset/native representation not validated. | Replace with compiled upstream source plus validated asset/native handle. |
| `gleam/dict.size`     | package asset | Dict package asset/native representation not validated. | Replace with compiled upstream source plus validated asset/native handle. |

## Removed Library Dispatch

These entries are ordinary library behavior. Their runtime dispatch arms and
direct stdlib codegen paths have been removed.

Source-backed behavior now comes from the loaded `gleam_stdlib` dependency, with
private native helpers only where upstream source calls bodyless externals.

Removed here means the public stdlib dispatch path is gone.

It does not mean every generic dependency call already has Wasm behavior coverage;
that remaining work belongs to monomorphized dependency emission.

| Entry                     | Replaced by                                         | Removal status                                                       |
| ------------------------- | --------------------------------------------------- | -------------------------------------------------------------------- |
| `gleam/bool.compare`      | Upstream source body.                               | Runtime table and direct codegen dispatch deleted.                   |
| `gleam/bool.negate`       | Upstream source body.                               | Runtime table and direct codegen dispatch deleted.                   |
| `gleam/bool.to_string`    | Upstream source body.                               | Runtime table and direct codegen dispatch deleted.                   |
| `gleam/float.compare`     | Upstream source body with `gleam/order`.            | Runtime table and direct codegen dispatch deleted.                   |
| `gleam/float.max`         | Upstream source body.                               | Runtime table and direct codegen dispatch deleted.                   |
| `gleam/float.min`         | Upstream source body.                               | Runtime table and direct codegen dispatch deleted.                   |
| `gleam/float.negate`      | Upstream source body.                               | Runtime table and direct codegen dispatch deleted.                   |
| `gleam/float.to_string`   | Upstream source wrapper plus private native helper. | Runtime table and direct codegen dispatch deleted.                   |
| `gleam/function.compose`  | Upstream source body.                               | Runtime table dispatch and registry-backed lowering adapter deleted. |
| `gleam/function.constant` | Upstream source body.                               | Runtime table and direct codegen dispatch deleted.                   |
| `gleam/function.flip`     | Upstream source body.                               | Runtime table dispatch and registry-backed lowering adapter deleted. |
| `gleam/function.identity` | Upstream source body.                               | Runtime table and direct codegen dispatch deleted.                   |
| `gleam/int.to_string`     | Upstream source wrapper plus private native helper. | Runtime table and direct codegen dispatch deleted.                   |
| `gleam/list.fold`         | Upstream source body.                               | Runtime table dispatch and registry-backed lowering adapter deleted. |
| `gleam/list.length`       | Upstream source body.                               | Runtime table and direct codegen dispatch deleted.                   |
| `gleam/list.map`          | Upstream source body.                               | Runtime table dispatch and registry-backed lowering adapter deleted. |
| `gleam/list.reverse`      | Upstream source body.                               | Runtime table and direct codegen dispatch deleted.                   |
| `gleam/option.map`        | Upstream source body.                               | Runtime table dispatch and registry-backed lowering adapter deleted. |
| `gleam/result.map`        | Upstream source body.                               | Runtime table dispatch and registry-backed lowering adapter deleted. |

## Completed Deletion Slices

Completed entries from the first source-backed deletion slice:

1. `gleam/bool.negate`
2. `gleam/list.fold`
3. `gleam/list.map`
4. `gleam/option.map`
5. `gleam/result.map`

Completed entries from the second source-backed scalar deletion slice:

1. `gleam/bool.to_string`
2. `gleam/float.compare`
3. `gleam/float.max`
4. `gleam/float.min`
5. `gleam/float.negate`
6. `gleam/function.identity`
7. `gleam/list.length`
8. `gleam/list.reverse`

Completed entries from the final scalar and registry-retained deletion slice:

1. `gleam/bool.compare`
2. `gleam/float.to_string`
3. `gleam/function.compose`
4. `gleam/function.constant`
5. `gleam/function.flip`
6. `gleam/int.to_string`
7. registry-backed lowering adapters for `gleam/list.{fold,map}`,
   `gleam/option.map`, `gleam/result.map`, and
   `gleam/function.{compose,flip}`

All scalar library-source runtime dispatch entries are now deleted.

The numeric `to_string` source wrappers use private `__regulus_native` helpers
for bodyless upstream externals; those helpers are runtime conversion primitives,
not public stdlib dispatch arms.

## Next Removal Candidates

- `gleam/dict.*`: remove compiler-owned dict collection behavior after
  upstream source can use a validated package asset or narrow native handle.
- `gleam/dynamic/decode.{list,optional}`: keep primitive decoder
  representation only; move combinator behavior to compiled upstream source.
- `gleam/dynamic/decode.*` primitive constructors: keep only if the runtime
  still needs concrete decoder values. Remove source-expressible wrapping
  behavior when upstream decoder modules compile.
- `gleam/bit_array.*` and `gleam/string.*`: not immediate deletion
  candidates. They remain runtime primitives unless upstream source wraps
  smaller representation-level helpers.

Future deletion work should prove:

- upstream source compiles from `gleam_stdlib`
- behavior fixtures pass through Wasm execution
- linked debug dumps contain the `gleam_stdlib:gleam/<module>.<function>`
  source path where applicable
- linked debug dumps do not contain public `__stdlib_gleam_*` dispatch names
