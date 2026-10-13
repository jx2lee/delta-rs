# [Bug]: DataFusion parquet metadata_size_hint is not applied in delta-rs scans

- Issue: https://github.com/delta-io/delta-rs/issues/4864 (opened 2026-10-12, Rust binding)
- Linked PR: none (checked 2026-10-13)
- Verified against main `b597cf23`

## Summary

delta-rs copies the session's Parquet options into `TableParquetOptions` and hands them to
`ParquetSource::with_table_parquet_options`, but DataFusion keeps `metadata_size_hint` in a
separate field on `ParquetSource` that only `with_metadata_size_hint` sets. So the hint configured
through `datafusion.execution.parquet.metadata_size_hint` never reaches the Parquet reader, and
every footer read falls back to the default two-request pattern (8-byte tail, then the footer).

## Where

- `crates/core/src/delta_datafusion/table_provider/next/scan/mod.rs:977` builds the main scan source.
- `crates/core/src/operations/load_cdf.rs:615-623` builds three sources for CDF (cdc / add / remove).
- `crates/core/src/datafile/properties.rs:17` only clones `execution.parquet` into `global`.

## Why it is a real bug (verified by reading DataFusion 55.1.0)

`datafusion-datasource-parquet-55.1.0/src/source.rs`:

- `with_table_parquet_options` only assigns `self.table_parquet_options`.
- `metadata_size_hint` is its own field (`pub(crate) metadata_size_hint: Option<usize>`) used when the
  opener is created, and the only setter is `with_metadata_size_hint`.

DataFusion's own `ParquetFormat::create_physical_plan` (`file_format.rs:490-513`) does exactly what
the issue proposes: reads `global.metadata_size_hint` and calls `with_metadata_size_hint` when set.
delta-rs skipped that step.

## Repro (code-level)

There is no runtime behaviour change that is easy to assert from outside, because the field is
`pub(crate)` in DataFusion. The observable symptom from the issue is extra tiny S3 range requests
during footer reads. A unit test can still cover the fix by checking the number of object-store
`get_range` calls with and without the hint (see guide.md).
