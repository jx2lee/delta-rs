Branch: `fix/optimize-commit-finished-bins`

Two possible scopes; pick after maintainers answer on the issue.

## Option A (small): unordered buffering for compaction

Title:

```
fix: yield finished compaction bins out of order so they can be committed
```

Body:

```markdown
# Description

Compaction schedules bin rewrites with `.buffered(max_concurrent_tasks)`, which yields results in
plan order. A slow bin at the head of the window holds back every finished bin behind it, and
because `min_commit_interval` is only evaluated when the stream yields, those finished bins cannot
be committed. Z-order already uses `buffer_unordered`.

Switch compaction to `buffer_unordered` so completed bins are committed as soon as the commit
interval elapses, independent of their neighbours. Bin planning order (newest files first for
`PreserveLocality`) is unchanged; only the order in which completed rewrites are observed differs.

# Related Issue(s)

- closes #4749

# Documentation

N/A
```

## Option B (larger): keep ordering, add a timer-driven commit

Title:

```
fix: commit buffered optimize actions when min_commit_interval elapses
```

Body: as above, but explain that the commit loop now selects between `stream.next()` and a
`tokio::time::interval(min_commit_interval)` tick so buffered actions are flushed even while the
stream is blocked on a slow bin.

Checklist: `cargo fmt --all`, `cargo clippy -p deltalake-core --features datafusion --tests`,
`cargo test -p deltalake-core --features datafusion --test it_datafusion optimize`, `git commit -s`.
