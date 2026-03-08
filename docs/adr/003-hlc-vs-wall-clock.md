# ADR-003: HLC vs Wall Clock for Timestamp Ordering

## Status

Accepted

## Context

AionDB assigns timestamps to every record at write time. These timestamps are used
for temporal queries (`AS OF`, `FOR PERIOD OF`) and for ordering events across the
system. In a distributed cluster, timestamps must provide causal ordering — if event
A causes event B, B's timestamp must be greater than A's.

`std::time::SystemTime` (wall clock) is insufficient for distributed timestamp ordering
because:

- NTP corrections can move the clock backwards, violating monotonicity
- Clock skew between nodes means two nodes can assign the same or reversed timestamps
  to causally ordered events
- Leap seconds can cause discontinuities

Lamport clocks provide causal ordering but lose the physical time component, making
timestamps unreadable and incompatible with human-meaningful temporal queries.

## Decision

All timestamps in AionDB use `HLCTimestamp` — a Hybrid Logical Clock as described
in "Logical Physical Clocks and Consistent Snapshots in Globally Distributed Databases"
(Kulkarni et al., 2014).

`HLCTimestamp` is defined as `(physical: u64, logical: u16)` where:

- `physical` tracks wall clock time in milliseconds, never going backwards
- `logical` is a counter that increments when two events share the same physical tick,
  ensuring strict monotonicity within and across nodes

Every timestamp in AionDB — `valid_from`, `valid_to`, `transaction_from`,
`transaction_to` — is an `HLCTimestamp`. Use of `std::time::SystemTime` directly
is forbidden in library code.

On send/receive of any message between nodes, the receiver advances its HLC by
taking `max(local, received)` and incrementing the logical counter if needed.

## Consequences

Pros:

- causal ordering across nodes: eventB caused by event A is guaranteed to have a later timestamp regardless of which node wrote it
- solves the problem with system time inconsistence

Cons:

- logical counter overflow - u16 gives 65535 ticks per millisecond. That introduces a limitation - node can process up to 65535 events per millisecond at maximum.
- clock drift must be bounded - HLC tolerates drift up to a configured epsilon. Nodes that drift too far must be fenced.

## Alternatives Considered

**`std::time::SystemTime` (wall clock)**

Rejected because NTP drift, leap seconds, and clock skew between nodes mean wall clock timestamps cannot provide causal ordering.

**Lamport clocks**

Lamport clock is just monotonically increasing number. It is not possible to convert it directly to physical time which makes impossible to query the data by physical or logical time.

**Hybrid Logical Clock with millisecond physical component**

Ms precision is sufficient for financial workloads. Financial settlement events occur on the order of seconds/ minutes, not sub-milliseconds. Sub-ms precision would require u64 physical + larger logical parts with no benefit.
