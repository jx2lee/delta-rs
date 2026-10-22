- PR: delta-io/delta-rs PR 4885 (opened 2026-10-15, merged as `2b656390` on 2026-10-15; DCO / semantic-release checks passed; closes #3997)
- Branch: `jx2lee/fix/unity-tolerant-table-models` (commit `4a369ee4`)
- Commit message: `fix(catalog-unity): tolerate unknown enum values in table responses`

Title:

```
fix(catalog-unity): tolerate unknown enum values in table responses
```

Body (as posted):

````markdown
# Description

`GetTableResponse` and `ListTableSummariesResponse` are untagged enums, so any field that fails to
decode is reported as `data did not match any variant of untagged enum ...`. `TableType` and
`DataSourceFormat` reject values Databricks has added since they were written (e.g. `FOREIGN`,
`METRIC_VIEW`), which makes the whole request fail.

- Add the currently documented `TableType` values: `FOREIGN`, `EXTERNAL_SHALLOW_CLONE`,
  `MANAGED_SHALLOW_CLONE`, `METRIC_VIEW`.
- Make the existing `Undefined` variant of `TableType` and `DataSourceFormat` the `#[serde(other)]`
  fallback, so values this crate doesn't know yet decode to `Undefined` instead of failing.

# Related Issue(s)

- closes #3997

# Documentation

- https://docs.databricks.com/api/workspace/tables/get
````

Not in this PR (left as possible follow-ups): `ColumnTypeName` still lacks `VARIANT` / `GEOGRAPHY` /
`GEOMETRY`, `Table.properties` still has no `#[serde(default)]`, and the opaque untagged-enum error
message is unchanged.

Checklist: `cargo fmt --all`, `cargo clippy -p deltalake-catalog-unity --tests`,
`cargo test -p deltalake-catalog-unity`, `git commit -s`.
