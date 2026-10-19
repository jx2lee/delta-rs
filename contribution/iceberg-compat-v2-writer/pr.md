Four PRs, in order. PR 0 is independent of Iceberg (file an issue first); ask on #3299 before PR 1–3 (see `plan.md` section 6).

Verified locally on upstream main `f67f1210` (2026-10-19) with all four applied: 6 new tests, `deltalake-core` lib (1360) / `it_datafusion` (167) / `it` (49) suites, fmt, clippy. Iceberg reader: Delta Spark 4.4.1 UniForm converted the table and pyiceberg 0.12 read it back, both directions (`plan-integration.md`).

Branches on `origin` (jx2lee/delta-rs), split from `prototype.diff` on 2026-10-20, each tested on its own (fmt, clippy on touched files, the tests it adds; PR 3 also the full lib and `it_datafusion` suites):

| PR | branch | base | commit | per-branch diff |
|---|---|---|---|---|
| 0 | `jx2lee/fix/arrow-schema-view-types` | upstream main `f67f1210` | `c5db2e93` | 4 files, +128/−19 |
| 1 | `jx2lee/feat/materialize-partition-columns` | upstream main `f67f1210` | `f7814766` | 8 files, +159/−11 |
| 2 | `jx2lee/feat/column-mapping-nested-ids` | PR 1 | `337f98a1` | 4 files, +319/−22 |
| 3 | `jx2lee/feat/iceberg-compat-v2` | PR 2 | `3e948a19` | 7 files, +204/−2 |

PR 2's `test_iceberg_compat_v2_writes_nested_field_ids` checks the nested ids and the parquet `field_id`s only (the partition column is still stripped there); PR 3 extends the same test with the protocol, materialization and tag assertions and adds the rejection test. Open PR 2 and PR 3 against upstream `main` with "stacked on PR n" in the body, or rebase them once the one below is merged. Upstream main had not moved past `f67f1210` when the branches were pushed.

---

## PR 0

- Issue first (title draft): `delete/update/merge/optimize write view and dictionary types into the parquet ARROW:schema`
- Branch: `jx2lee/fix/arrow-schema-view-types`
- Title: `fix: write plain arrow types into the parquet ARROW:schema footer`

````markdown
# Description

Files rewritten by delete, update, merge and optimize carry the DataFusion scan types in their
`ARROW:schema` footer: `Utf8View`/`BinaryView` for strings and binaries, and a dictionary for a
partition column that is kept in the file. Files written by `write` carry `Utf8`. The parquet
encoding is identical, but a reader that restores the footer (pyarrow, and pyiceberg on top of
it) gets `string_view` from one file and `string` from the next and cannot combine them:

    ArrowTypeError: Unable to merge: Field name has incompatible types: string vs string_view

and filters on a single rewritten file fail with `Function 'array_filter' has no kernel
matching input types (string_view, bool)`. #4579 reported the same symptom; #4589 fixed the
compression part only.

`FileArrowWriter::try_new` now writes the footer itself, from the file schema with
`Utf8View → Utf8`, `BinaryView → Binary` and dictionaries replaced by their value type
(`without_view_types`), and asks the arrow writer to skip its own. Batches are not cast.
Both the parallel and the serial writer go through this.

Two optimize tests downcast the read-back column to `StringViewArray`; they now expect
`StringArray`, which is what a file written by `write` already gave.

Verified by hand: pyiceberg could not scan a table after a delta-rs delete and can after this
change (`contribution/iceberg-compat-v2-writer/plan-integration.md` in my fork has the steps).

# Related Issue(s)

- closes #<new issue>

# Documentation

N/A
````

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
- Every `add` gets `tags: {"ICEBERG_COMPAT_VERSION": "2"}`, the tag Delta Spark puts on files
  it writes to such a table. Without it, Spark's `REORG TABLE ... APPLY (UPGRADE UNIFORM)`
  rewrites every delta-rs file. Not in PROTOCOL.md; easy to drop if you prefer.

Verified with Delta Spark 4.4.1: it accepts the table, its UniForm hook generates Iceberg
metadata whose field ids are exactly the column-mapping and nested ids delta-rs assigned, the
data files are registered in place, and pyiceberg reads them (nested types, decimals,
timestamps, partition pruning). Spark can append to the table and delta-rs reads and writes it
afterwards.

Out of scope: `icebergCompatV1`, generating Iceberg metadata (only a Spark commit refreshes
it), the legacy writers. Known limitation: `OPTIMIZE` on column-mapped tables is still refused
by delta-rs, so it is on these tables too. Kernel has a `TODO` for V1/V2 validation; the checks
here are kept minimal so they can move there.

# Related Issue(s)

- closes #3299

# Documentation

Table properties added to the docs table (TODO).
````
