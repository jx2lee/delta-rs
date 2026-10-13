# OPTIMIZE compaction uses ordered buffered(): a slow bin blocks committing finished bins; min_commit_interval only checked on yield

- Issue: https://github.com/delta-io/delta-rs/issues/4749 (opened 2026-09-20, Rust)
- Linked PR: none (checked 2026-10-13). Reporter offered a PR but asked maintainers for direction first.
- Related, different issue: #4750 (hang at 16 tasks, awaiting feedback)
- Verified against main `b597cf23`

## Summary

`OptimizeBuilder::execute` (`crates/core/src/operations/optimize.rs`):

- Compaction arm: `.buffered(max_concurrent_tasks)` at line 864 (ordered).
- Z-order arm: `.buffer_unordered(max_concurrent_tasks)` at line 906.
- Commit loop (lines 927-975): `min_commit_interval` is only evaluated after `stream.next().await`
  returns, so finished bins behind a slow head bin cannot be committed until the head finishes.

With a 1 GiB bin taking ~36 min, the reporter saw no commit for ~37 min while five finished outputs
existed. An external cancel then orphaned all of them.

## History (why this is not just a typo)

`git log -S".buffered(max_concurrent_tasks)"` shows the ordered variant was introduced by
`86d97cca fix: preserve compact locality by default` (2026-03-27, Ethan Urbanski). Before that
commit compaction used `buffer_unordered`. The commit added `PlannerStrategy::PreserveLocality`
and "newest files first" bin planning (`optimize.rs:1219`), so the ordering may have been chosen so
that commits land in planning order. The issue explicitly asks whether that is intentional.

## Reproduction (deterministic, no S3 needed)

The behaviour is purely about stream ordering, so a unit test can show it: make the first bin's
rewrite future sleep while later bins complete, set `min_commit_interval` very small, and observe
that no commit happens until the slow one finishes. See guide.md for a test sketch that wraps the
log store (pattern already used in `crates/core/tests/it_datafusion/command_optimize.rs`,
`OperationTrackingLogStore`).
