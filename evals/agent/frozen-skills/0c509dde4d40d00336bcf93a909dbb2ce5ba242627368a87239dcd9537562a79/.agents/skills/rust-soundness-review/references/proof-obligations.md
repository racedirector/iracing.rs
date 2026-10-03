# Proof obligations

Write the theorem before inspecting isolated unsafe blocks. For each consumer,
list its required validity, initialized extent, provenance, alignment, aliasing,
lifetime, thread, ordering and cleanup properties. Trace all safe producers and
mutations; include failed construction, cancellation, reconnect and deserialization.
A local SAFETY comment is a claim to check against those paths.

| Property | Establishment to look for |
| --- | --- |
| Representation | Exact size plus valid bit patterns/discriminants for the wire type |
| Semantics | Field domain checks beyond representation validity |
| Layout | Checked cross-field arithmetic, overlap and frame geometry |
| Source bounds | Extent checked against this source and still valid at access |
| Alignment | Actual address, not merely a repr annotation |
| Lifetime | Owner remains live across references, threads and cancellation |
| Snapshot | Producer protocol and copy/recheck sequence protect the consumed bytes |

A safe constructor that asks callers to preserve an unsafe memory invariant does
not eliminate that obligation. Explain where it moved and whether safe callers
can violate it. A passing unit test, Miri run or proposed typestate design cannot
certify unreviewed paths. A valid control should receive no invented finding.
