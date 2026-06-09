# Collector families

An arena answers one memory question: where does the next object go? A
collector answers a different question: which old objects can be reused?

Collector design is a set of tradeoffs about pauses, throughput, memory
overhead, pointer stability, implementation size, and how much type information
the compiler can give the runtime. This section surveys the common families a
language runtime can choose from.

## Reference counting

Reference counting stores a count of references to each object. Copying a
pointer increments the count. Overwriting or dropping a pointer decrements it.
When the count reaches zero, the runtime can reclaim the object.[^recycling]

The appeal is prompt reclamation. When the last reference goes away, the object
can usually be freed immediately. That can help runtimes with scarce memory or
objects that hold external resources.

The costs are paid on ordinary program operations. Assignments, function calls,
returns, and field updates may need count changes. Simple reference counting
also does not collect cycles, because two dead objects can keep each other's
counts above zero. Deferred, weighted, and one-bit variants reduce some costs,
but they add their own rules.

## Mark-sweep tracing

Tracing collectors start from roots: stack slots, globals, registers, host
handles, and other places that can directly hold live pointers. The collector
follows pointers from those roots and marks every reachable object. A sweep pass
then reclaims unmarked objects.[^recycling]

Mark-sweep does not move live objects. That makes raw pointers stable, which is
useful for simple ABIs and host inspection. The tradeoff is fragmentation. Over
time, free memory can be split into many small holes, and allocating large
objects can become harder even when total free memory is high.

The runtime also needs to know which words are pointers. Exact tracing uses
compiler-provided metadata for stack slots and object fields. Conservative
tracing treats pointer-shaped words as possible pointers, which can keep dead
objects alive.

## Copying and compacting collectors

Copying collectors move live objects into another region and reclaim the old
region as a whole. Compacting collectors move objects to reduce gaps inside a
region. Both approaches improve locality and reduce fragmentation.[^recycling]

Moving objects requires pointer updates. Every pointer in an object field,
local, global, stack slot, and host handle must be found and rewritten. That
makes copying and compacting collectors depend on accurate root metadata and
clear rules for pointers outside the guest runtime.

The payoff can be large. Allocation after copying can look like arena
allocation again: bump a pointer through contiguous free space.

## Generational collectors

Generational collectors divide objects by age. New objects are allocated in a
young generation, often called a nursery. Objects that survive collections are
promoted to older generations. The design relies on the generational
hypothesis: most young objects die young.[^generational]

Collecting a small nursery can be much cheaper than scanning the whole heap.
The hard part is tracking pointers from old objects into young objects. A
runtime usually maintains remembered sets with write barriers. A write barrier
is code that runs when a pointer field changes, so the collector can later find
cross-generation references without scanning all old objects.

Generational collection is often paired with copying collection for the nursery
and mark-sweep or mark-compact collection for older objects.

## Incremental and concurrent collectors

Basic tracing can pause the program until marking and sweeping finish.
Incremental collectors split that work into smaller steps. Concurrent
collectors do some work while the program continues to run.[^recycling]

These collectors improve latency, but they need more bookkeeping. While the
collector is marking, the program can allocate new objects and change pointers.
The runtime must preserve the collector's invariants with barriers, safe
points, or cooperation from the compiler.

The main question is no longer only "is this object live?" The runtime also has
to know whether the collector and program agree about the state of the object
graph at each point where execution can continue.

## WebAssembly GC

WebAssembly GC adds Wasm-level aggregate and reference types for languages that
want the engine to manage objects rather than storing every managed value in
linear memory.[^wasmgc] A compiler targeting those features can represent some
runtime values as Wasm GC objects instead of raw byte layouts.

That changes the boundary. The engine can collect GC references, but the
compiler must map source values into the available Wasm GC types and still
define how values cross imports, exports, JavaScript, WASI, or another host
environment. Linear memory may still be useful for byte buffers, custom ABIs,
or compatibility with non-GC targets.

## Choosing a collector

There is no collector that dominates every workload. Reference counting gives
prompt reclamation but charges pointer operations. Mark-sweep preserves object
addresses but can fragment memory. Copying collectors make allocation cheap
after collection but move objects. Generational collectors exploit common object
lifetime patterns but require barriers. Incremental and concurrent collectors
reduce pauses by increasing runtime complexity.

For a compiler book, the important lesson is that memory management depends on
representation. Object headers, field layouts, root metadata, host handles, and
ABI rules decide which collectors are possible.

[^recycling]: Memory Management Reference, "Recycling techniques": https://www.memorymanagement.org/mmref/recycle.html

[^generational]: Memory Management Reference, "generational garbage collection": https://www.memorymanagement.org/glossary/g.html#generational-garbage-collection

[^wasmgc]: WebAssembly GC proposal overview: https://github.com/WebAssembly/gc/blob/main/proposals/gc/Overview.md
