# ADR-002: Workspace Crate Structure

## Status

Proposed

## Context

AionDB is a multi-concern system: storage engine, consensus protocol, SQL parsing,
HTTP server, and embeddable client library. These concerns have different release
cycles, different dependency graphs, different stability guarantees, and different
intended consumers.

The decision is how to structure this as a Cargo project: monolithic single crate,
or a Cargo workspace with multiple crates.

Forces at play:

- **Dependency isolation** — the consensus layer must not depend on the SQL parser.
  The client library must not pull in the server. Accidental coupling between layers
  is a maintenance liability.
- **Compile times** — Cargo can parallelise compilation of independent crates.
  A monolith recompiles everything on any change.
- **Distribution** — `aiondb-client` is intended for publication on crates.io as a
  standalone library. It must be independently versioned and have a minimal dependency
  surface.
- **Interface discipline** — crate boundaries enforce explicit public APIs. Code that
  should be internal cannot accidentally leak across layers.
- **Distributed architecture** — `aiondb-consensus` must exist as a distinct crate
  from day one so the interface between consensus and storage is a deliberate design
  decision, not an afterthought.

## Decision

AionDB is structured as a Cargo workspace with five crates:

| Crate              | Responsibility                                          |
| ------------------ | ------------------------------------------------------- |
| `aiondb-core`      | Storage engine, temporal model, MVCC, HLC, indexes      |
| `aiondb-consensus` | Raft consensus — stubbed Phase 0, implemented Phase 10  |
| `aiondb-server`    | axum HTTP server, query routing, cluster membership API |
| `aiondb-client`    | Embeddable Rust client library, published to crates.io  |
| `aiondb-sql`       | SQL lexer, parser, AST, query planner                   |

Dependency graph (arrows = "depends on"):

```
aiondb-server ──→ aiondb-core
aiondb-server ──→ aiondb-consensus
aiondb-server ──→ aiondb-sql
aiondb-sql    ──→ aiondb-core
aiondb-client ──→ aiondb-core        (types only)
aiondb-consensus ──→ aiondb-core     (NodeId, LogEntry types)
```

`aiondb-core` has no dependencies on any sibling crate.
`aiondb-consensus` has no dependency on `aiondb-sql` or `aiondb-server`.

## Consequences

**Positive:**

- `aiondb-client` can be published to crates.io independently with a minimal `Cargo.toml`
  and no transitive dependency on server, consensus, or SQL parsing.
- `pub(crate)` vs `pub` has real meaning — internal types cannot leak across layers at
  compile time, not just by convention.
- The `aiondb-consensus` crate boundary makes the consensus/storage interface a formal
  contract. Swapping the stub for real Raft in Phase 10 requires no changes to
  `aiondb-core` or `aiondb-server`.
- Independent crates compile in parallel — changes to `aiondb-sql` do not trigger
  recompilation of `aiondb-consensus`.
- Different server implementations (e.g. gRPC alongside HTTP) can share `aiondb-core`
  without modification.

**Negative:**

- Path dependencies require `cargo deny` configuration to prevent wildcard versions.
- Cross-crate refactors (moving a type between crates) touch multiple `Cargo.toml` files
  and constitute a breaking change for downstream consumers of the affected crate.
- More boilerplate: each crate needs its own `Cargo.toml`, error types, and re-exports.

## Alternatives Considered

**Monolithic single crate** — rejected. `aiondb-client` would pull in `axum`, `tokio`
server dependencies, and the SQL parser as transitive dependencies for any consumer.
The consensus/storage interface would be an internal module boundary enforced only by
discipline, not the compiler. Publishing to crates.io would be impractical.

**Two-crate split (`aiondb-core` + `aiondb-app`)** — rejected. Consolidating server,
client, SQL, and consensus into one `aiondb-app` crate preserves the library/binary
split but loses all the benefits of interface discipline between those layers. The
consensus interface remains an internal convention rather than a crate-level contract.
