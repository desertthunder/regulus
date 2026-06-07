# Runtime memory and value semantics tasks

## Goal

Complete runtime memory management and runtime value semantics.

## Tasks

### Memory management

- [x] Choose a resettable bump arena with checked `memory.grow`.
- [ ] Implement checked allocation and heap growth for every allocator path.
- [ ] Define allocation failure as a structured runtime panic payload.
- [ ] Keep managed objects non-moving until instance reset or arena reset.
- [ ] Document host pointers as borrowed and stable until reset.
- [ ] Add Wasmtime tests for growth, failed growth, and pointer stability.

### Equality and ordering

- [ ] Implement recursive structural equality for strings, bit arrays, lists,
      tuples, records, custom values, results, and options.
- [ ] Define equality behavior for closures, opaque values, errors, and panics.
- [ ] Implement ordering helpers for all orderable Gleam values.
- [ ] Diagnose unsupported ordering combinations before WAT assembly.
- [ ] Add nested equality and ordering execution tests.

### Inspection and debug rendering

- [x] Implement deterministic string inspection with escaping.
- [x] Implement debug rendering for scalar and managed values.
- [x] Render nested lists, tuples, records, custom values, bit arrays, errors,
      panics, and opaque placeholders.
- [x] Add runtime tests for debug rendering.

### Error and panic payloads

- [x] Define payload layouts for panic, todo, assert, match failure, and runtime
      errors.
- [x] Materialize payloads where helpers need host-readable failures.
- [x] Add host-readable panic/error inspection tests.

## Done when

Runtime helper behavior matches documented Gleam semantics and memory management
has explicit, tested allocation and lifetime rules.
