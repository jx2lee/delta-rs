Branch: `jx2lee/feat/write-preserve-order` (off upstream main `21f4376c`)

Comment on the issue first (see guide.md section 2); the maintainers closed #2659 with "cannot
guarantee order", so say what the option does and does not promise before opening the PR.

Title:

```
feat: add preserve_order to the write operation
```

Body:

````markdown
# Description

`write_deltalake` (and `WriteBuilder`) does not keep the row order of a multi-batch input: the
physical optimizer round-robins the single-partition source over `target_partitions` streams
(`enable_round_robin_repartition`), and `write_streams` drains those streams concurrently into
one writer channel, so batches interleave. A pre-sorted stream lands unsorted, which defeats
clustering by a sort key (narrow row-group min/max).

This adds an opt-in:

- `WriteBuilder::with_preserve_order(bool)`. When set, the session used for planning is rebuilt
  with `enable_round_robin_repartition = false` and `repartition_file_scans = false`, so the
  input keeps its partitioning. A `RecordBatchReader` or a `MemTable` is one stream and is
  written as it arrives. Inputs with several partitions keep their per-partition order only.
- `write_deltalake(..., preserve_order=False)` in Python, threaded through
  `_internal.write_to_deltalake` and `RawDeltaTable.write`.

Nothing after the planner needed to change: for unpartitioned writes all streams already feed
a single `DeltaWriter` task, so what the option gives up is the scan-side parallelism, not the
encoding. Default behaviour is unchanged.

Tests: a Rust unit test writes 64 ascending batches with `target_partitions = 4` and reads the
data file back with the parquet reader; with `preserve_order` the rows are monotonic, and a
second test documents that the default path reorders them. A Python test streams 64 batches
through `write_deltalake(preserve_order=True)`.

# Related Issue(s)

- closes #4874

# Documentation

`write_deltalake` docstring describes the option and its cost.
````

Checklist: `cargo fmt --all`, `cargo clippy -p deltalake-core --features datafusion --tests`,
`cargo test -p deltalake-core --features datafusion --lib preserve_order`, python
`make develop && uv run --no-sync pytest tests/test_writer.py -k preserve_order`, `git commit -s`.
