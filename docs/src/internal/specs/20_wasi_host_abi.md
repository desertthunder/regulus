# WASI host ABI

Regulus accepts a WASI-oriented target, but WASI host behavior is not complete.
This spec keeps that target explicit instead of mixing it into JavaScript or
Wasmtime-specific work.

## Scope

The WASI host ABI should define how compiled Gleam code uses WASI capabilities
such as stdout, stderr, clocks, random bytes, files, and environment data.

It should also define which stdlib members are supported on WASI and which fail
with target-specific diagnostics.

## Initial profile

The first useful WASI profile should cover:

- module instantiation with WASI imports
- `gleam/io.print` and `gleam/io.println` through `fd_write`
- scalar exports for smoke tests
- string exports through existing data/length helpers
- target validation for unsupported host imports

## Managed values

WASI uses the same low-level Wasm ABI as other targets: scalars are raw Wasm
values and managed values are borrowed pointers into guest memory. Host tools
may read managed strings and structured values through exported helpers.

## Deferred work

Full filesystem, environment, clocks, random values, sockets, and process APIs
are deferred until examples need them. Unsupported WASI capabilities should fail
before byte emission.

## Active tasks

See [WASI host ABI tasks](../tasks/21_wasi_host_abi.md).
