Branch: `fix/unity-tolerant-table-models`

Title:

```
fix(catalog-unity): tolerate unknown enum values in table responses
```

Body:

```markdown
# Description

`GetTableResponse` is an untagged enum, so any field of `Table` that fails to decode is reported as
`data did not match any variant of untagged enum GetTableResponse`. The table models are strict:
`TableType`, `ColumnTypeName` and `DataSourceFormat` reject values Databricks has added since they were
written (`FOREIGN`, `VARIANT`, `GEOGRAPHY`, `ICEBERG`, ...), and `Table.properties` is required although
it is omitted when empty.

- Add the currently documented values to the three enums and an `#[serde(other)] Unknown` variant so
  future additions degrade gracefully instead of failing the whole request.
- Default `Table.properties` to an empty map.
- Decode the success case first and include the serde error in the returned error so users see which
  field failed.

# Related Issue(s)

- closes #3997

# Documentation

- https://docs.databricks.com/api/workspace/tables/get
```

Checklist: `cargo fmt --all`, `cargo clippy -p deltalake-catalog-unity --tests`,
`cargo test -p deltalake-catalog-unity`, `git commit -s`.
