# write_deltalake does not keep the row order of a multi-batch input; opt-in to preserve it

- Issue: https://github.com/delta-io/delta-rs/issues/4874 (opened 2026-10-14, Python, `enhancement`, priority "Low - nice to have")
- Linked PR: none (checked 2026-10-21). No maintainer comment yet. Reporter will test, will not submit a PR.
- Related: #2659 (closed 2024, "cannot guarantee order", pyarrow engine), pola-rs/polars#26912 comment
- Verified against upstream main `21f4376c` (2026-10-21)

## Summary

The reporter clusters tables by writing rows sorted on a key, so each file's row groups get narrow
min/max statistics. `write_deltalake` with a `RecordBatchReader` of 8192-row batches writes the
rows out of order (31 breaks in 4M rows, roughly one every 131k rows, on 16 cores). A single-chunk
table comes back sorted, but that means holding everything in memory. They ask for an opt-in such
as `preserve_order=True`.

## Where the order is lost

The write runs the DataFusion physical optimizer over the sink plan:

```text
crates/core/src/operations/write/mod.rs      into_future: session.create_physical_plan(&sink_plan)
crates/core/src/operations/write/plan.rs     the sink plan has a cast projection over the source
crates/core/src/operations/write/execution.rs
    write_data_plan        unpartitioned: execute_stream_partitioned → write_streams
    write_streams          one worker task per input stream, all feeding ONE mpsc channel,
                           drained by ONE DeltaWriter task
```

- The Python reader becomes a `LazyTableProvider`, a single-partition source. `EnforceDistribution`
  sees a projection with non-trivial exprs above a 1-partition child and inserts
  `RepartitionExec(RoundRobinBatch(target_partitions))` (`enable_round_robin_repartition`, default
  true, `target_partitions` = cores). A `MemTable` input (`with_input_batches`) is additionally
  split by `repartition_file_scans` (`MemorySourceConfig::repartitioned`).
- `write_streams` then has N streams and spawns N workers that push into one channel. The batches
  interleave there. 16 cores × 8192 rows = 131k rows between breaks matches the report.
- Everything after the channel is sequential (one `DeltaWriter`, `PartitionWriter` rolls files in
  arrival order), so stopping the repartitioning is enough to keep the order.

This also means the round-robin only parallelises the scan/cast side; parquet encoding was already
funnelled through one writer task for unpartitioned writes, so opting out costs little there.

## Fix shape

`WriteBuilder::with_preserve_order(bool)`; when set, the resolved `SessionState` is rebuilt with
`enable_round_robin_repartition = false` and `repartition_file_scans = false` before planning. Python
gets `write_deltalake(..., preserve_order=False)` threaded through `_internal.write_to_deltalake`
and `RawDeltaTable.write`. Partitioned tables are covered too: a 1-partition input skips
`repartition_by_partition_columns`, and hash repartitioning keeps per-output order anyway.

In Rust the caller can already get this by passing a session with the two options off via
`with_session_state`; Python has no session access, which is the case for the kwarg.

## Reproduction

Rust unit test on `21f4376c` (guide.md section 5): 64 batches × 1024 ascending `i64` through
`DeltaTable::write` with `target_partitions = 4`:

```
default write:   15 order breaks in 65536 rows   (test_write_default_reorders_multi_batch_input)
preserve_order:  0 breaks, 65536 rows monotonic  (test_write_preserve_order_keeps_input_order)
```

The issue's own script (4M rows in 8192-row batches through `write_deltalake`, dev build of the
Python binding on the same commit):

```
preserve_order=False: 47 breaks in 4194304 rows, 1 file(s)
preserve_order=True:  0 breaks in 4194304 rows, 1 file(s)
```

Prototype: `+184/-2` over 5 files (`write/mod.rs` 136 of them, mostly the tests), Rust and
Python tests, fmt, clippy, ruff, mypy clean; the 93 tests of `operations::write` still pass.
