Branch: `fix/parquet-metadata-size-hint`

Commit / PR title (conventional commits, lower case):

```
fix: propagate parquet metadata_size_hint to delta scans
```

PR body:

```markdown
# Description

`ParquetSource::with_table_parquet_options` only stores the `TableParquetOptions`; DataFusion keeps
the footer prefetch hint in a separate `metadata_size_hint` field that is set only through
`with_metadata_size_hint`. The `next` table provider and `load_cdf` never called it, so
`datafusion.execution.parquet.metadata_size_hint` (or `DATAFUSION_EXECUTION_PARQUET_METADATA_SIZE_HINT`)
had no effect on delta-rs scans and the reader fell back to the default footer read pattern.

Apply the hint from `global.metadata_size_hint` after constructing each `ParquetSource`, which is
what DataFusion's own `ParquetFormat::create_physical_plan` does.

# Related Issue(s)

- closes #4864

# Documentation

https://docs.rs/datafusion/latest/datafusion/config/struct.ParquetOptions.html#structfield.metadata_size_hint
```

Checklist before opening:

- `cargo fmt --all`
- `cargo clippy -p deltalake-core --features datafusion --tests`
- `cargo test -p deltalake-core --features datafusion metadata_size_hint`
- Commit with `git commit -s` (DCO sign-off is required by CONTRIBUTING.md)
- Mention in the PR which parts were AI-assisted and how you verified them (CONTRIBUTING.md "Using AI-generated code")
