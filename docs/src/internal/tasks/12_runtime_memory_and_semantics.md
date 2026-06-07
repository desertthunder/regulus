# Runtime memory tasks

## Goal

Complete runtime memory management and allocation failure behavior.

## Tasks

### Memory management

- [x] Choose a resettable bump arena with checked `memory.grow`.
- [ ] Implement checked allocation and heap growth for every allocator path.
- [ ] Define allocation failure as a structured runtime panic payload.
- [ ] Keep managed objects non-moving until instance reset or arena reset.
- [ ] Document host pointers as borrowed and stable until reset.
- [ ] Add Wasmtime tests for growth, failed growth, and pointer stability.

## Done when

Memory management has explicit, tested allocation, growth, failure, and lifetime
rules.
