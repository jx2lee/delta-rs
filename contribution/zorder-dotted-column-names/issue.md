# [Feature]: Align z-order column parsing with ColumnName semantics

- Issue: https://github.com/delta-io/delta-rs/issues/4363 (opened 2026-04-12)
- Linked PR: none (checked 2026-10-13). Someone commented `take` on 2026-04-12 and nothing followed in six months,
  so ask in the issue before starting (or just comment `take` again; the bot reassigns).
- Verified against main `b597cf23`

## Summary

Z-order resolves column paths by naive `split('.')` (`crates/core/src/operations/optimize.rs:778` in
`read_zorder`, `:1288` in `validate_zorder_column`). A column whose name literally contains a dot
cannot be z-ordered at all, and there is no escape syntax. The kernel already ships
`delta_kernel::expressions::ColumnName` with backtick escaping (`` `a.b` `` = literal, `n.a` = nested),
and delta-rs uses it elsewhere (`delta_datafusion/data_validation.rs`).

## Repro (Rust, run 2026-10-13)

Table with columns `a.b: int` and `n: struct<a: int>`:

```text
zorder ["a.b"]   : ERR Z-order column "a.b": field "a" not found in schema
zorder ["`a.b`"] : ERR Z-order column "`a.b`": field "`a" not found in schema
zorder ["n.a"]   : OK files_added=1
```

So nested paths work, but a literal dotted name fails both unescaped and escaped.

## Also affected

`build_zorder_plan` (`optimize.rs:1322`) compares `partition_keys.contains(col)` with the raw string, so
the "cannot z-order on partition column" check also only works for top-level names.
