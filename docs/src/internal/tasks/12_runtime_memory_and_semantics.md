# Runtime memory and value semantics tasks

## Goal

Complete runtime memory management and runtime value semantics.

## Tasks

### Memory management

- [ ] Decide between bump-only, resettable arena, reference counting, tracing
      GC, host-owned arenas, or another documented strategy.
- [ ] Define heap growth and allocation failure behavior.
- [ ] Define object lifetime, ownership, and movement rules.
- [ ] Implement heap bounds checks or memory growth where needed.
- [ ] Add tests for allocation growth, exhaustion, and lifetime assumptions.

### Equality and ordering

- [ ] Implement recursive structural equality for strings, bit arrays, lists,
      tuples, records, custom values, results, and options.
- [ ] Define equality behavior for closures, opaque values, errors, and panics.
- [ ] Implement ordering helpers for all orderable Gleam values.
- [ ] Diagnose unsupported ordering combinations before WAT assembly.
- [ ] Add nested equality and ordering execution tests.

### Inspection and debug rendering

- [ ] Implement deterministic string inspection with escaping.
- [ ] Implement debug rendering for scalar and managed values.
- [ ] Render nested lists, tuples, records, custom values, bit arrays, errors,
      panics, and opaque placeholders.
- [ ] Add snapshot tests for debug rendering.

### Error and panic payloads

- [ ] Define payload layouts for panic, todo, assert, match failure, and runtime
      errors.
- [ ] Materialize payloads where helpers need host-readable failures.
- [ ] Add host-readable panic/error inspection tests.

## Done when

Runtime helper behavior matches documented Gleam semantics and memory management
has explicit, tested allocation and lifetime rules.
