# Runtime memory tasks

## Goal

Complete runtime memory management and allocation failure behavior.

## Tasks

### Memory management

- [x] Choose a resettable bump arena with checked `memory.grow`.
- [x] Implement checked allocation and heap growth for every allocator path.
- [x] Define allocation failure as a structured runtime panic payload.
- [x] Keep managed objects non-moving until instance reset or arena reset.
- [x] Document host pointers as borrowed and stable until reset.
- [x] Add Wasmtime tests for growth, failed growth, and pointer stability.

## Done when

Memory management has explicit, tested allocation, growth, failure, and lifetime
rules.
