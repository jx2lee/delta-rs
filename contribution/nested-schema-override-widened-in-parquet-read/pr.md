Branch: `fix/nested-schema-override-parquet-read`

Title:

```
fix: keep nested schema overrides when building the kernel scan schema
```

Body:

```markdown
# Description

`DeltaScanConfig::schema` can narrow nested struct children, and `ProjectedScanContract::scan_schema`
reflects that, but `KernelScanPlan::try_new_with_contract` projected the kernel logical schema only by
root column names. The kernel scan therefore still returned the full struct, and the derived
`parquet_read_schema` asked Parquet for every sibling leaf under each selected root.

Build the kernel logical read schema from `contract.scan_schema` (keeping Delta metadata such as
column-mapping ids by resolving each nested field against the table schema) so that the kernel
physical schema, its per-file transforms and the Parquet read schema all describe the same narrowed
nested fields. Scans without an override are unchanged.

Tested with the `table_with_null_stats_in_notnull_struct` fixture (plain) and a column-mapped
partitioned table: the Parquet schema now contains only the requested leaves, and the query output
is unchanged.

# Related Issue(s)

- closes #4678

# Documentation

N/A
```

Checklist: `cargo fmt --all`, `cargo clippy -p deltalake-core --features datafusion --tests`,
`cargo test -p deltalake-core --features datafusion next::scan`, Python `uv run pytest tests -k schema`,
`git commit -s`.
