# ADR-004: RowId Design — Composite vs UUID v7

## Status

Accepted

## Context

AionDB is a distributed database. In a single-node system, a local monotonic counter
is sufficient for record identity. In a cluster, records can be written concurrently
on multiple nodes — a purely local counter is not globally unique.

`RowId` must be:

- Globally unique across all nodes without coordination at write time
- Traceable back to the originating node for audit and debugging
- Monotonically ordered within a node
- Stable across node restarts and cluster rebalances

## Decision

`RowId` is defined as a composite `(node_id: u64, timestamp: HLCTimestamp)`.

- `node_id` is assigned by the operator or cluster bootstrap. It is unique across
  all nodes in the cluster and stable for the lifetime of the node.
- `timestamp` is the `HLCTimestamp` assigned at write time on the originating node.

Uniqueness is guaranteed because HLC increments the logical counter on every local
event — no two writes on the same node can share the same `HLCTimestamp`. Combined
with a unique `node_id`, the composite is globally unique without coordination.

## Consequences

Pros:

- allows to trace each record up to the specific node in the specific moment of time

Cons:

- requires proper `node_id` initialization during the cluster bootstrap

## Alternatives Considered

**Per-table monotonic counter**

This approach is rejected as the sequence could lose uniqueness in case of node crash.
Also the approach requires to store the sequence which lowers the performance.

**Per-node WAL counter**

This approach is rejected as number of items in WAL could change after cluster rebalance so that it is not necessarily monotonically increasing number. E.g. rows migrated onto a node during the rebalance arrive with the sequence already assigned by another node - a local WAL counter has no knowledge of the imported sequences, creating collision risk.

**UUID v7**

UUID v7 approach works perfectly, however chosen approach allows to trace each record to the specific node and moment in time it was created.

**`(node_id, HLCTimestamp, random: u32)` fallback**

This was considered as a fallback for (node_id, HLCTimestamp) assuming that several records could arrive on the same node in the same moment of time. However, the HLCTimestamp design guarantees the uniqueness of it's value in that case.
