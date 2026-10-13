# Setting Universal Format (UniForm) table properties not possible

- Issue: https://github.com/delta-io/delta-rs/issues/3299 (opened 2025-03-04, label enhancement, Python)
- Linked PR: none (checked 2026-10-13). Maintainer comment (2025-03) said "once we switch to kernel-rs we can
  support parsing these". The kernel switch has happened; the properties are still rejected.
- Verified against main `b597cf23`

## Summary

`DeltaTable.create(..., configuration={"delta.enableIcebergCompatV2": "true",
"delta.universalFormat.enabledFormats": "iceberg"})` fails with

```
Kernel: Generic delta kernel error: Error parsing property 'delta.enableIcebergCompatV2':'true'
```

The same happens with `dt.alter.set_table_properties(...)`.

## Root cause

- `crates/core/src/table/config.rs:18` `TableProperty` is delta-rs' own allow-list of known `delta.*`
  keys. It has no variant for `delta.enableIcebergCompatV1/V2/V3` or `delta.universalFormat.enabledFormats`.
- `crates/core/src/kernel/models/actions.rs:472-481` `apply_properties_to_protocol` parses every key
  with `key.parse::<TableProperty>()`; an unknown key with `raise_if_not_exists = true` (the default
  for `create` and `set_table_properties`) returns the error above.
- `delta_kernel` already knows the keys (`kernel/src/table_properties/mod.rs:44-46`
  `ENABLE_ICEBERG_COMPAT_V1/V2/V3`), but delta-rs does not use that parser here (there is a `TODO`
  at `actions.rs:372`).

Workaround users can use today: `raise_if_key_not_exists=False` on create. That skips the error but
also does not set the `icebergCompatV2` writer feature that the protocol requires.

## Repro (Rust, run 2026-10-13)

```text
parse enableIcebergCompatV2: false
parse universalFormat.enabledFormats: false
create result: Some(Kernel { source: Generic("Error parsing property 'delta.enableIcebergCompatV2':'true'") })
```

from

```rust
let config: HashMap<String, Option<String>> = [
    ("delta.enableIcebergCompatV2".to_string(), Some("true".to_string())),
    ("delta.universalFormat.enabledFormats".to_string(), Some("iceberg".to_string())),
].into_iter().collect();
DeltaTable::new_in_memory().create()
    .with_column("x", DataType::INTEGER, true, None)
    .with_configuration(config).await
```
