Branch: `feat/zorder-column-name-parsing`

Title:

```
feat: parse z-order columns with kernel ColumnName semantics
```

Body:

```markdown
# Description

Z-order split column paths on `.` to support nested struct fields (#2007). That left no way to
z-order on a column whose name itself contains a dot: both `a.b` and `` `a.b` `` fail with
`field "a" not found in schema`.

Parse each z-order input with `delta_kernel::expressions::ColumnName` so backtick escaping works the
same way as everywhere else in Delta (`` `a.b` `` is one field, `n.a` is a nested path), and carry
the parsed path through `validate_zorder_column` and `read_zorder`. The public API stays
`Vec<String>`; plain nested paths behave exactly as before.

# Related Issue(s)

- closes #4363

# Documentation

https://github.com/delta-io/delta-kernel-rs/blob/main/kernel/src/expressions/column_names.rs
```

Checklist: `cargo fmt --all`, `cargo clippy -p deltalake-core --features datafusion --tests`,
`cargo test -p deltalake-core --features datafusion zorder`, `git commit -s`.
