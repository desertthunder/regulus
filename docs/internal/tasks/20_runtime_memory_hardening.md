# Runtime memory hardening tasks

## Goal

Make runtime allocation and host memory inspection safe enough for examples and
longer-running tests.

## Tasks

### Allocation behavior

- [x] Add overflow checks for allocation size and alignment arithmetic.
- [x] Define whether dynamic memory may grow at runtime.
- [x] Add heap-limit checks before bump allocation succeeds.
- [x] Add deterministic failure behavior for allocation failure.
- [x] Test allocation at page boundaries and near overflow limits.

We should allow dynamic memory growth by default for the current bump
allocator phase. Growth keeps non-trivial examples usable before reclamation
exists, but it is bounded by an explicit maximum and fails deterministically.
A fixed-memory policy can still be added later as a target or host profile for
constrained environments.

### Runtime object validation

- [ ] Add a runtime helper inventory grouped by allocation, managed values,
      closures, equality, debug, dynamic values, and host adapters.
- [ ] Validate object tags, sizes, arity, and field indexes in exported reader
      helpers.
- [ ] Add tests for invalid host reader calls.
- [ ] Document which helper failures trap and which return sentinel values.

### Ownership and lifetimes

- [ ] Document when host-held managed pointers remain valid.
- [ ] Document rules for host-provided managed pointers.
- [ ] Define how opaque host handles interact with runtime ownership.
- [ ] Reject unsupported pointer or handle ownership shapes during ABI
      validation.

### Deferred reclamation

- [ ] Record the first supported reclamation strategy: none, arena reset,
      reference counting, or garbage collection.
- [ ] Add tests that long-running examples fail clearly before exhausting
      memory, or grow memory when growth is enabled.

## Done when

Allocation failures, host reader misuse, and pointer lifetime assumptions are
specified, tested, and visible in diagnostics or documented traps.
