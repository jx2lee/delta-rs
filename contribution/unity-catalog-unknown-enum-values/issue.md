# [Bug]: unity catalog request GetTableResponse error

- Issue: https://github.com/delta-io/delta-rs/issues/3997 (opened 2025-12-18, labels bug, binding/rust, unity-catalog)
- Linked PR: PR 4885 (open, opened 2026-10-15) — covers `TableType` / `DataSourceFormat` only
- Verified against main `b597cf23`

## Summary

Opening `uc://catalog.schema.table` against Databricks fails with

```
reqwest::Error { kind: Decode, source: Error("data did not match any variant of untagged enum GetTableResponse") }
```

although `curl .../api/2.1/unity-catalog/tables/<name>` returns the table fine. The reporter gave no
response body, but the models in `crates/catalog-unity/src/models.rs` explain the failure class:

1. `GetTableResponse` is `#[serde(untagged)] { Success(Table), Error(ErrorResponse) }`
   (`models.rs:88-95`). Untagged enums throw away the inner serde error, so *any* problem decoding
   `Table` surfaces as this opaque message.
2. `Table`, `ColumnInfo` and the enums are strict:
   - `TableType` (`models.rs:306`) knows `MANAGED, EXTERNAL, VIEW, MATERIALIZED_VIEW, STREAMING_TABLE`
     but Databricks also returns `FOREIGN`, `EXTERNAL_SHALLOW_CLONE`, `MANAGED_SHALLOW_CLONE`, `METRIC_VIEW`.
   - `ColumnTypeName` (`models.rs:403`) lacks `VARIANT`, `GEOGRAPHY`, `GEOMETRY`.
   - `DataSourceFormat` (`models.rs:274`) lacks newer formats (e.g. `ICEBERG`, `DELTA_UNIFORM_ICEBERG`, `TERADATA_FORMAT`, ...).
   - `Table.properties` (`models.rs:357`) has `skip_serializing_if` but no `#[serde(default)]`, so a response
     without `properties` fails with `missing field properties`.
   Any one of these makes the whole response undecodable. A `VARIANT` column in the table is the most likely
   trigger for a Databricks user in late 2025.

## Repro (serde unit test, run 2026-10-13)

Same `Table` JSON, varying one value:

```text
table_type=MANAGED type_name=STRING   : Success
table_type=MANAGED type_name=VARIANT  : DECODE ERR data did not match any variant of untagged enum GetTableResponse
table_type=FOREIGN type_name=STRING   : DECODE ERR data did not match any variant of untagged enum GetTableResponse
table_type=MANAGED type_name=GEOGRAPHY: DECODE ERR data did not match any variant of untagged enum GetTableResponse
```

Decoding `Table` directly shows the real cause, e.g.
`unknown variant VARIANT, expected one of BOOLEAN, BYTE, ... TABLE_TYPE`.
Omitting `properties` gives `missing field properties`.
