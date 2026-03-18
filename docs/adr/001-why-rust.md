# ADR-001: Why Rust for a Database Engine

## Status

Proposed

## Context

AionDB is a distributed, strongly-consistent, bitemporal database engine targeting
financial-grade workloads. The language choice has long-term consequences across
correctness, performance, and operational safety.

The primary candidates were Rust, Go, C++, and Java. The requirements that drove
the decision:

- **Predictable latency** — Raft election timeouts are typically 150–300ms. A GC
  pause of comparable duration can cause spurious leader re-elections and cluster
  instability. Financial SLAs do not tolerate stop-the-world pauses.
- **Concurrency safety** — the WAL, the in-memory index, and the HLC are shared
  mutable state accessed by concurrent readers and writers. Data races here produce
  silent corruption, not crashes.
- **Low-level access** — a production storage engine requires memory-mapped files,
  custom allocators, and lock-free structures. These require controlled unsafe
  operations.
- **Throughput** — serialisation, hashing, and compaction are CPU-bound hot paths
  that benefit from zero-overhead abstractions.

## Decision

Rust is the implementation language for all AionDB crates.

Key reasons:

1. **No garbage collector.** Rust's ownership model provides deterministic memory
   reclamation with no stop-the-world pauses. This is the single most important
   property for Raft correctness and financial latency SLAs.

2. **`unsafe` is localised and auditable.** C++ is implicitly unsafe everywhere —
   every pointer dereference, every cast, every buffer access. Rust confines unsafe
   operations to explicitly marked blocks with mandatory `// SAFETY:` invariants.
   The surface area of unverified code is minimal and reviewable.

3. **Fearless concurrency.** The borrow checker enforces at compile time that shared
   mutable state is accessed under correct synchronisation. Data races across the
   WAL writer, HLC, and index are compile errors, not runtime bugs found in
   production.

4. **Zero-cost abstractions.** Iterators, generics, and async futures compile to
   equivalent machine code as hand-written C. There is no JIT warmup, no boxing,
   no hidden allocation.

5. **Async ecosystem.** `tokio` provides an M:N async runtime enabling thousands of
   concurrent client connections on a small thread pool. `async/await` integrates
   with the type system — futures are `Send`-checked at compile time.

6. **Rich systems crate ecosystem.** `serde`, `tokio`, `axum`, `parking_lot`,
   `thiserror`, `criterion` — production-quality crates with no equivalent in C++
   or Go for this specific workload combination.

## Consequences

**Positive:**

- Compile-time elimination of data races, use-after-free, and buffer overflows across the entire codebase
- Deterministic latency — no GC pauses affecting Raft timeouts or financial SLAs
- `unsafe` surface area is minimal, auditable, and enforced by convention (`// SAFETY:` comments)
- Async runtime (`tokio`) scales to thousands of concurrent connections with a small thread pool

**Negative:**

- Steeper learning curve than Go or Java — borrow checker requires discipline upfront
- Longer compile times compared to Go
- Smaller talent pool than Java or Go for hiring

## Alternatives Considered

**C++** — rejected. Comparable performance to Rust but implicitly unsafe throughout.
Buffer overflows, memory leaks, and segmentation faults are possible anywhere in the
codebase, not just in auditable `unsafe` blocks. No borrow checker means data races
require discipline and tooling (ThreadSanitizer) rather than compiler enforcement.

**Go** — rejected. Garbage collector introduces non-deterministic stop-the-world pauses.
Even with tuning, GC pauses in the tens of milliseconds are possible under memory
pressure — unacceptable when Raft election timeouts are 150–300ms. Additionally, nil
pointer dereferences are runtime panics, not compile-time errors.

**Java** — rejected. GC pauses are worse than Go under high-throughput write workloads.
JVM startup time is unsuitable for short-lived cluster tooling. No safe mechanism for
memory-mapped I/O or custom allocators required by the storage engine. Null pointer
dereferences are runtime exceptions, not compile-time errors.
