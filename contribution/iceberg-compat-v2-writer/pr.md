Three PRs, in order. Ask on #3299 first (see `plan.md` section 6).

Verified locally on upstream main `f67f1210` (2026-10-19) with all three applied: 4 new tests, `deltalake-core` lib (1358) / `it_datafusion` (167) / `it` (49) suites, fmt, clippy. Not verified: reading the resulting table with an Iceberg reader.

---

## PR 1

- Branch: `jx2lee/feat/materialize-partition-columns`
- Title: `feat: support the materializePartitionColumns writer feature`

````markdown
# Description

Implements the `materializePartitionColumns` writer feature (PROTOCOL.md "Materialize Partition
Columns") and the `delta.writePartitionColumnsToParquet` table property: when either is set,
every data file written by the DataFusion write paths (write, merge, update, delete, optimize,
including change data files) keeps the partition columns, with the value recorded in the file's
`partitionValues`. On column-mapped tables they are written under their physical name and field
id like the other columns.

- `materialize_partition_columns(&TableConfiguration)` decides once per operation; the feature
  (supported == active), `icebergCompatV1/V2` (enabled) and the property all route through it.
- `WriterConfig` carries the flag; `file_schema()` and `write_partition` stop stripping the
  partition columns when it is set. Optimize computes its file schema the same way.
- File statistics are still not collected for partition columns (`stats_from_metadata`
  already skips them), as the protocol asks.
- `materializePartitionColumns` is added to the supported writer features (datafusion builds).
- The legacy `RecordBatchWriter` / `JsonWriter` always strip partition columns, so they now
  refuse tables that require materialization instead of writing non-conforming files.

Readers are unchanged: partition values keep coming from the log.

# Related Issue(s)

- related #3299 (first step towards `icebergCompatV2`)

# Documentation

Table property added to the docs table (TODO).
````

## PR 2

- Branch: `jx2lee/feat/column-mapping-nested-ids`
- Title: `feat: assign and write nested field ids for icebergCompatV2 tables`

````markdown
# Description

Accepts the UniForm table properties (`delta.enableIcebergCompatV1`,
`delta.enableIcebergCompatV2`, `delta.universalFormat.enabledFormats`) in `TableProperty`, and
when a table is created with `delta.enableIcebergCompatV2 = true`:

- `CREATE TABLE` asks kernel's `assign_column_mapping_metadata` to also allocate
  `delta.columnMapping.nested.ids` for array elements and map keys/values.
- The column-mapping write node reads those ids and attaches them as `PARQUET:field_id` to the
  list element and map key/value fields, so the parquet writer stores them in `field_id`
  (PROTOCOL.md "Writer Requirements for IcebergCompatV2").

This PR does not enable the feature in the protocol yet; that is the next PR.

# Related Issue(s)

- related #3299

# Documentation

N/A
````

## PR 3

- Branch: `jx2lee/feat/iceberg-compat-v2`
- Title: `feat: support the icebergCompatV2 writer feature`

````markdown
# Description

With partition columns materialized and nested field ids written, delta-rs now meets the writer
requirements of `icebergCompatV2`, so this PR turns the property into the feature:

- `delta.enableIcebergCompatV2 = true` sets writer version 7 and adds `icebergCompatV2` to
  `writerFeatures`. It requires column mapping (`name` or `id`) and is rejected together with
  `delta.enableIcebergCompatV1` or `delta.enableDeletionVectors`.
- The table schema is checked against the V2 type allow-list at `CREATE TABLE` (variant, void
  and nanosecond timestamps are rejected). Column-mapped tables already refuse later schema
  changes, so no other entry point needs the check.
- `icebergCompatV2` is added to the supported writer features; `add_feature` knows the name.
- Enabling the property on an existing table via `SET TBLPROPERTIES` is refused, because the
  nested ids are assigned to the schema at creation.

Out of scope: `icebergCompatV1`, generating Iceberg metadata, the legacy writers. Kernel has a
`TODO` for V1/V2 validation; the checks here are kept minimal so they can move there.

# Related Issue(s)

- closes #3299

# Documentation

Table properties added to the docs table (TODO).
````
