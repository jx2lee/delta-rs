Branch: `feat/uniform-table-properties`

Title:

```
feat: accept iceberg compat and universal format table properties
```

Body:

```markdown
# Description

`TableProperty` did not know `delta.enableIcebergCompatV1`, `delta.enableIcebergCompatV2` or
`delta.universalFormat.enabledFormats`, so creating a table with UniForm properties (or setting them
with `set_table_properties`) failed with
`Error parsing property 'delta.enableIcebergCompatV2':'true'`.

Add these keys to `TableProperty` so they are stored as plain table properties.

This PR intentionally does **not** add the `icebergCompatV1`/`icebergCompatV2` writer features to the
protocol. Both are writer features that delta-rs does not implement (they are not in the supported
writer feature set used by `ProtocolChecker::can_write_to`), so enabling them would make the table
unwritable by delta-rs itself. Per the protocol, the property alone does not make the feature active.
`delta.enableIcebergCompatV3` is left out because it is still a proposed protocol RFC.

delta-rs does not produce Iceberg metadata.

# Related Issue(s)

- closes #3299

# Documentation

- https://docs.delta.io/latest/delta-uniform.html
- https://github.com/delta-io/delta/blob/master/PROTOCOL.md#iceberg-compatibility-v2
- https://github.com/delta-io/delta/blob/master/PROTOCOL.md#supported-features
```

Checklist: `cargo fmt --all`, `cargo clippy -p deltalake-core --features datafusion --tests`,
`cargo test -p deltalake-core --features datafusion apply_properties`, Python test in
`python/tests/test_table.py` or `test_create.py`, `git commit -s`.

방향(guide 3-3 의 A/B 안)을 이슈에서 메인테이너에게 먼저 확인한 뒤 본문을 맞출 것. 위 본문은 A 안 기준.
