# Runtime representation

WebAssembly provides numbers, functions, tables, and linear memory. Gleam values
such as strings, lists, tuples, records, closures, and custom types need explicit
runtime representations before they can be compiled correctly.

## Values to represent

- `String`
- lists
- tuples
- records
- custom types and tagged unions
- closures and function values
- boxed scalar values where needed
- panic/todo/error payloads

## Memory model

The runtime design should specify:

- allocation strategy
- object headers and tags
- string encoding
- list layout
- tuple and record layout
- custom-type constructor layout
- ownership or garbage-collection strategy
- host ABI for imported and exported values

## Constraints

The representation should be simple to inspect in tests, stable enough for code
generation, and compatible with Wasmtime and browser execution.
