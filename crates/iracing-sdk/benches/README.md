# SDK benchmarks

These Criterion targets answer different performance questions. Choose the
narrowest target that matches the behavior under investigation; results from
different targets are not interchangeable measurements of “frame latency.”

| Target | Measures | Does not measure |
| --- | --- | --- |
| `var_data_extraction` | Individual scalar, array, bitfield, and bounds-error operations | Whole-frame or delivery-pipeline cost |
| `frame_construction` | Byte-buffer ownership, `FramePacket` construction, `Arc` cloning, and tick operations | Telemetry decoding or acquisition |
| `adapter_performance` | Dynamic lookup and typed adapter construction from a prepared packet | Provider, connection, or subscription work |
| `aggregate_frame_parsing` | Fresh owned outputs for all variables, a representative consumer, and all scalars | Frame acquisition or end-to-end delivery |
| `session_parsing` | Session YAML sanitization and typed deserialization from a checked-in live snapshot | Acquisition, shared-memory reads, or update publication |
| `telemetry_delivery_e2e` | Deterministic in-process provider-to-adapted-subscriber delivery | IBT I/O, Windows shared memory, or simulator pacing |
| `subscriber_fanout` | One shared SDK stream with service-side fan-out versus one SDK stream per client, using heterogeneous requested-field projections | WS/gRPC serialization, sockets, client backpressure, or network I/O |
| `live_reader_acquisition` | Deterministic direct-copy, unchanged-tick, stable acquisition, and forced-retry costs for an 8,586-byte live layout | Win32 mapping, event waits, simulator pacing, or delivery |
| `live_frame_latency` | Manual live subscriptions with a running simulator | Stable, deterministic CI performance |

Run all compile-safe targets with:

```text
cargo bench -p iracing-sdk --features benchmark --no-run
```

Run one target with:

```text
cargo bench -p iracing-sdk --features benchmark --bench <target>
```

The source-level documentation at the top of each benchmark defines its setup
and timed boundaries, allocation behavior, throughput unit, and interpretation
limits.

## Captured-schema fixture

Cross-platform decoding and construction targets use the checked-in live-schema
capture to generate deterministic, type-correct bytes. Frame size, offsets, and
aggregate variable counts belong to that capture and may change with an iRacing
build. Fixture generation, schema validation, metadata ordering, and sentinel
checks happen before timed loops unless a target explicitly documents otherwise.

The fixture provides a realistic layout, not recorded driving values. A result
therefore describes work on a prepared in-memory frame on the benchmark machine;
it is not automatically an end-to-end live telemetry result.

## Deterministic delivery pipeline

`telemetry_delivery_e2e` requires neither iRacing nor Windows shared memory. Its
controlled provider copies the full fixture into a new `FramePacket`, passes it
through a production delivery policy, and adapts it into the shared 47-field
consumer at 1, 4, and 16 subscribers.

`latest_paced` releases one source frame only after every benchmark subscriber
has consumed the previous latest snapshot. Its Criterion elements are completed
subscriber adaptations, so dividing by the subscriber count gives source-frame
throughput.

`latest_burst_8` offers eight source updates before subscribers consume the
latest snapshot. Seven versions per burst are intentionally replaced. Its
configured elements count offered source frames multiplied by subscribers; they
do not represent the smaller number of adapted outputs that survive coalescing.

`ondemand_acknowledged` preserves every frame and advances only after all active
subscriptions poll again to acknowledge their prior frame. Its elements are
completed subscriber adaptations.

`ondemand_slow_ack` polls every fast subscriber once, deterministically
withholds the final subscriber's acknowledgement, and only then permits that
subscriber to release the shared cursor. It uses logical polling gates rather
than wall-clock sleeps.

The latency diagnostic timestamps immediately before the provider copies the
fixture into a packet and samples after the adapter reaches a subscriber. It
prints p50, p95, and p99 nanoseconds separately from Criterion throughput runs
because timestamp collection perturbs the fast path. The Criterion `completed`
case is only an execution marker, not the latency measurement.

`telemetry_delivery_allocations` is a separate diagnostic executable using a
current-thread runtime and counting global allocator. It reports allocations
and allocated bytes per source frame and subscriber delivery; those values are
not timing results because allocator instrumentation perturbs the hot path.

Runtime creation, schema validation, subscription construction, and task
spawning occur before each reported duration. Cancellation and joining occur
after elapsed time is captured.

## Subscriber fan-out

`subscriber_fanout` compares two service architectures while holding projection
work constant. Every simulated client receives a fresh owned projection of 12
scalar fields. The field plans rotate through the captured schema so clients do
not all request the same variables, and all schema lookup happens before timing.

`shared_stream` creates one SDK `DynamicFrame` subscription and projects that
frame once for every client. `stream_per_client` creates one SDK subscription
per client, consumes every subscription for each source frame, and then applies
the corresponding projection. Both cases use production latest-value delivery
and a current-thread Tokio runtime.

Criterion measures 1 through 512 clients. A separate diagnostic exponentially
scales farther and reports average source-frame time, source frames per second,
and utilization of a 60 Hz frame budget. The diagnostic is useful for locating
the order of magnitude where synchronous, fully drained fan-out stops keeping
up; use the Criterion cases for statistically sampled comparisons.

Projection vectors are fresh output values suitable for later serialization,
but serialization, protocol framing, network writes, per-client queues, slow
clients, and multi-threaded service scheduling are outside the measured
boundary.

## Manual live benchmark

`live_reader_acquisition` is portable and deterministic. It uses an in-memory
`RandomAccessSource` with the 8,586-byte frame length from the checked-in live
schema. Accepted-frame cases include allocation of the owned `FrameBuffer`;
the unchanged-tick case proves and measures the path that performs no frame
allocation or frame-region copy. Direct-copy and accepted cases report frame
bytes per second, while unchanged reports observations per second. The forced
retry changes the source immediately after the first frame copy and accepts the
second attempt. It intentionally excludes shared-memory mapping, event waits,
Tokio scheduling, provider conversion, and subscriber delivery.

Run it with:

```text
cargo bench -p iracing-sdk --features benchmark --bench live_reader_acquisition
```

`live_frame_latency` is Windows-only and requires an active iRacing session.
Several cases await source-paced frames, so results include simulator cadence,
Tokio scheduling, and operating-system wake-up behavior. Follow the target’s
module documentation before interpreting or comparing its output.

## Comparing results

- Compare like-for-like case names, schema revisions, build profiles, machines,
  and allocation policies.
- Treat Criterion throughput according to the unit documented by that case;
  backing-frame bytes are not valid throughput for a selected-field workload.
- Distinguish additional work caused by schema growth from slower normalized
  decoder performance.
- Performance values are reports, not correctness thresholds or proof that an
  implementation is optimally tuned.
