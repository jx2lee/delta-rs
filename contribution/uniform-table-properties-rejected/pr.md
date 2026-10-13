Branch: `feat/uniform-table-properties`

Title:

```
feat: accept iceberg compat and universal format table properties
```

Body:

```markdown
# Description

`TableProperty` did not know `delta.enableIcebergCompatV1`, `delta.enableIcebergCompatV2`,
`delta.enableIcebergCompatV3` or `delta.universalFormat.enabledFormats`, so creating a table with
UniForm properties (or setting them with `set_table_properties`) failed with
`Error parsing property 'delta.enableIcebergCompatV2':'true'`.

Add the four keys to `TableProperty`, and when `delta.enableIcebergCompatVn = true` enable the
matching `icebergCompatVn` writer feature (writer version 7) in `apply_properties_to_protocol` and
`move_table_properties_into_features`, the same way `delta.enableDeletionVectors` is handled.
`delta.universalFormat.enabledFormats` is stored as a plain property, as in Delta Spark.

delta-rs does not produce Iceberg metadata; this change only lets the properties and the protocol
feature be written so that other engines (Spark/Databricks UniForm) can take over.

# Related Issue(s)

- closes #3299

# Documentation

- https://docs.delta.io/latest/delta-uniform.html
- https://github.com/delta-io/delta/blob/master/PROTOCOL.md#iceberg-compatibility-v2
```

Checklist: `cargo fmt --all`, `cargo clippy -p deltalake-core --features datafusion --tests`,
`cargo test -p deltalake-core --features datafusion table_properties`, Python test in
`python/tests/test_table.py` or `test_create.py`, `git commit -s`.
