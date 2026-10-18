# [Bug]: DeltaScanNext widens nested schema overrides for Parquet reads

- Issue: https://github.com/delta-io/delta-rs/issues/4678 (opened 2026-08-28, labels bug, binding/rust)
- Linked PRs: #4685 (closed 2026-10-04 by rtyler: "asymmetry in the effort required to babysit this PR"),
  #4729 (closed 2026-09-17 as duplicate). Both closed without merging, so the issue is open for a new attempt.
- Verified against main `b597cf23`

## Summary

`DeltaScanConfig::schema` lets a caller narrow the schema, including nested struct children.
`ProjectedScanContract::scan_schema` honours that (only `s.l`), but `KernelScanPlan::try_new_with_contract`
builds the kernel scan from a **root-level** projection (`kernel_logical_schema.project(&names)`,
`plan.rs:301-314`) and then derives `parquet_read_schema` from `scan.physical_schema()`
(`plan.rs:315-318`). The kernel physical schema still has the full struct, so the Parquet reader
reads every sibling leaf (`s.b` as well as `s.l`).

## Repro (run 2026-10-13, test added to `plan.rs` tests module)

```text
scan_schema:         s: Struct([l: Int64])
parquet_read_schema: s: Struct([l: Int64, b: Boolean])   <-- widened
assertion `left == right` failed
```

Test code (fixture `crates/test/tests/data/table_with_null_stats_in_notnull_struct`):

```rust
let override_schema = Arc::new(Schema::new(vec![arrow_schema::Field::new(
    "s",
    DataType::Struct(vec![Arc::new(arrow_schema::Field::new("l", DataType::Int64, false))].into()),
    false,
)]));
let config = DeltaScanConfig::default().with_schema(override_schema.clone());
let scan_plan = KernelScanPlan::try_new(snapshot, None, &[], &config, None)?;
assert_eq!(scan_plan.contract.scan_schema, override_schema);   // passes
assert_eq!(scan_plan.parquet_read_schema, override_schema);    // fails
```

## What the maintainers said on the closed PRs (must be addressed by any new PR)

ethan-tyler, review on #4685:

- `contract.scan_schema` is a *logical* schema; `parquet_read_schema` is *physical*. Deriving one from the
  other directly ignores partition-column removal and column-mapping physical names.
- A test must prove the query returns the expected rows **and** that Parquet receives only the selected
  nested leaf. Asserting the private field alone "does not protect the read path".
- The fixture used has no partition columns and no column mapping, so equality there proves little.
- Pruning the parquet schema while the kernel per-file transforms still use the wider logical schema
  misaligns reader output and transforms. Either keep them aligned or retain the wider input where
  narrowing is not safe.

ethan-tyler on #4729: iterating over override `selection_fields` reorders the reader's children, but kernel
transforms expect the original order.

## Likely correct fix direction

Narrow the **kernel** scan schema (nested), not just the parquet schema after the fact. If the logical
read schema handed to `ScanBuilder::with_schema` is already nested-pruned, kernel derives a nested-pruned
physical schema with column-mapping names, and its transforms are built from that same schema, so the
reader and the transforms stay aligned. See guide.md.
