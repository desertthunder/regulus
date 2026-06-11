# Runtime memory hardening tasks

## Goal

Make runtime allocation and host memory inspection safe enough for examples and
longer-running tests.

## Tasks

### Allocation behavior

- [ ] Add overflow checks for allocation size and alignment arithmetic.
- [ ] Define whether dynamic memory may grow at runtime.
- [ ] Add heap-limit checks before bump allocation succeeds.
- [ ] Add deterministic failure behavior for allocation failure.
- [ ] Test allocation at page boundaries and near overflow limits.

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
