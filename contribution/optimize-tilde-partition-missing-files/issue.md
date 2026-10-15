# optimize fails with "File selection contains N missing files" when a partition value contains `~`

- Issue: **not filed yet** (searched 2026-10-16: no issue/PR mentions this symptom). File the draft below first.
- Related (not duplicates):
  - #2146 path-encoding test umbrella
  - #4815 centralize path handling (lists `a~b` as a test value but doesn't report this bug)
  - #4646 vacuum leaves `%`/`~` partition files (PR #4658 open, different code path)
- Verified against upstream main `3604f18a` (2026-10-15)
- Regression: Python 1.5.0 works, **1.5.1 and later fail** (checked 1.4.2 / 1.5.0 / 1.5.1 / 1.6.0 / 1.6.1 / 1.6.6)

## Summary

`optimize` passes the files of each bin to the scan as a `FileSelection` (`optimize.rs:745` compact, `:769` z-order;
only compact was run, z-order goes through the same `provider_for`).
`FileSelection::from_adds` turns each `Add.path` (already percent-*decoded*) into an id with
`String::from(Path::from(path))` (`crates/core/src/delta_datafusion/table_provider/next/mod.rs:236`).
The scan side builds ids for active files from the *raw* log path (`path_raw()`) joined to the table root
(`scan/replay.rs:509`).

`object_store::Path::from` percent-encodes its own `INVALID` set, which includes `~`. delta-rs' log
encoding (`serde_path::INVALID` in `kernel/models/actions.rs`) and the partition-value encoding
(RFC 3986, `~` is unreserved) both leave `~` alone. So for `p=a~b/...` the selection id becomes
`p=a%7Eb/...` and the active id `p=a~b/...`. They never match, and every selected file is reported missing.

Other values work only by coincidence: for `a b` the decoded path is `p=a%20b/...`. `Path::from` turns it into
`%2520`, which happens to equal the raw log form. Characters that differ between the two encoding sets
(object_store encodes, delta-rs doesn't) are the trigger. Of those, only `~` survives partition encoding.

## Repro (Python, 1.6.6)

```python
import pyarrow as pa
from deltalake import DeltaTable, write_deltalake
for _ in range(2):
    write_deltalake("t", pa.table({"p": ["a~b"], "n": [1]}), partition_by=["p"], mode="append")
DeltaTable("t").optimize.compact()
# DeltaError: Generic DeltaTable error: Error during planning: File selection contains 2 missing files
#   (showing up to 10, redacted): file:///.../t/p=a%7Eb/part-00000-....snappy.parquet, ...
```

Reading, delete/update/merge and vacuum on the same table don't hit this error. Vacuum has its own
problem, #4646.

## Repro (Rust, upstream main `3604f18a`)

Added to `crates/core/tests/it_datafusion/command_optimize.rs` (reuses `setup_test`, `write`, `tuples_to_batch`):

```rust
#[tokio::test]
async fn repro_optimize_tilde_partition_value() -> Result<(), Box<dyn Error>> {
    for value in ["a~b", "a b", "2022-05-22"] {
        let context = setup_test(true).await?;
        let mut dt = context.table;
        let mut writer = RecordBatchWriter::for_table(&dt)?;
        write(&mut writer, &mut dt, tuples_to_batch(vec![(1, 2)], value)?).await?;
        write(&mut writer, &mut dt, tuples_to_batch(vec![(2, 3)], value)?).await?;
        let res = dt.optimize().await;
        println!("{value:?}: {:?}", res.as_ref().map(|(_, m)| m.num_files_removed).map_err(|e| e.to_string()));
    }
    Ok(())
}
```

```text
"a~b": Err("... File selection contains 2 missing files ...: file:///.../date=a%7Eb/part-00000-....parquet, ...")
"a b": Ok(2)
"2022-05-22": Ok(2)
```

Run: `cargo test -p deltalake-core --features datafusion --test it_datafusion repro_optimize_tilde -- --nocapture`
(needs the `dat/` test data; `make setup-dat` or symlink an existing checkout's `dat/`).

## Issue draft (to file)

Title: `optimize fails with "File selection contains N missing files" for partition values containing "~"`

```markdown
# Environment

**Delta-rs version**: 1.5.1 – 1.6.6, main (3604f18a). 1.5.0 works.
**Binding**: python and rust
**Environment**: local filesystem (macOS); the bug is in path-id normalization, not storage specific

# Bug

**What happened**:
`optimize.compact()` fails (z-order shares the same scan path) on a table partitioned by a string column whose value contains `~`:

    DeltaError: Generic DeltaTable error: Error during planning: File selection contains 2 missing files
    (showing up to 10, redacted): file:///.../t/p=a%7Eb/part-00000-....snappy.parquet, ...

The files exist at `p=a~b/...`, which is also what `add.path` says.

**What you expected to happen**: the partition is compacted like any other.

**How to reproduce it**:

    import pyarrow as pa
    from deltalake import DeltaTable, write_deltalake
    for _ in range(2):
        write_deltalake("t", pa.table({"p": ["a~b"], "n": [1]}), partition_by=["p"], mode="append")
    DeltaTable("t").optimize.compact()

**More details**:
`FileSelection::from_adds` builds the selection id with `object_store::Path::from(add.path)`, which
percent-encodes `~` (`%7E`). The scan builds ids for active files from the raw log path, where `~` stays
literal, so the ids never match. Values like `a b`, `a%b`, unicode or `/` are unaffected because
`Path::from` happens to reproduce the log encoding for them. Related: #2146, #4815.
```
