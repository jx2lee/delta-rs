- Branch: `jx2lee/fix/optimize-tilde-partition-path` (not created yet)
- Prerequisite: file the issue in `issue.md` first, then put its number in `closes #...`
- Fix verified locally on upstream main `3604f18a` (2026-10-16):
  - the repro passes for `a~b`, `a b`, `a%b`, `a%20b`, `a/b`, `한글`, `a+b`, `a#b`, `[a]` and a plain date
  - existing `file_selection` lib tests (29) and `command_optimize` integration tests (40) pass

Title:

```
fix: match optimize file selection ids for partition values containing "~"
```

Body:

````markdown
# Description

`FileSelection::from_adds` built the selection id from the decoded `Add.path` with
`object_store::Path::from`, which percent-encodes `~`. The scan builds ids for active files from the
raw log path, where `~` is kept literal (neither the log path encoding nor the RFC 3986 partition
value encoding escape it). For a partition value containing `~` the two ids never matched, so
`optimize` failed with `File selection contains N missing files`.

Re-encode the decoded path with the same encoder used when writing `add.path` to the log
(`serde_path::encode_path`), so the selection id is built from the same representation as the
active-file id. Other values already matched and are unaffected.

Added a regression test that compacts partitions with `~`, space, `%`, unicode and `/` in the value.

# Related Issue(s)

- closes #<new issue>
- related #2146, #4815

# Documentation

N/A
````
