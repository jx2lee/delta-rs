# 수정 가이드: write_deltalake 에 입력 순서 유지 옵션 추가 (#4874)

난이도: ★★☆☆☆ (핵심 로직 15줄, 나머지는 Rust/Python 배관)

## 1. 문제를 한 줄로

입력은 스트림 하나인데, DataFusion optimizer 가 그것을 코어 수만큼 round-robin 으로 쪼개고,
writer 가 그 스트림들을 동시에 비워서 하나의 채널에 넣기 때문에 배치가 섞입니다.

## 2. 코드 읽기

```text
crates/core/src/operations/write/mod.rs
  into_future            resolve_session_state(...) 로 SessionState 를 얻고
                         session.create_physical_plan(&sink_plan) 로 물리 플랜 생성 ← 여기서 RepartitionExec 삽입
crates/core/src/operations/write/plan.rs
  prepare_write          source 위에 cast projection → optimizer 가 "병렬화하면 이득" 으로 판단
crates/core/src/operations/write/execution.rs
  write_data_plan        unpartitioned: execute_stream_partitioned → write_streams
  write_streams          스트림마다 worker task → mpsc 채널 하나 → DeltaWriter task 하나
```

개념 메모:
- `LazyTableProvider`(Python 의 RecordBatchReader) 와 `MemTable`(`with_input_batches`) 은 파티션 1개짜리 소스입니다.
- DataFusion `EnforceDistribution` 규칙은 `enable_round_robin_repartition`(기본 true) 이면 파티션이 `target_partitions`
  보다 적은 입력 위에 `RepartitionExec(RoundRobinBatch(n))` 을 끼웁니다. `MemTable` 은 `repartition_file_scans`
  (기본 true) 로도 배치 단위로 쪼개집니다.
- `write_streams` 는 스트림 N개를 N개 task 로 동시에 읽어 채널 하나에 넣습니다. 섞이는 곳은 정확히 여기입니다.
- 채널 뒤는 `DeltaWriter` 하나가 순차로 받으므로 순서가 유지됩니다. 그래서 repartition 만 막으면 됩니다.

이슈의 "131k 행마다 끊김" = 16코어 × 8192행 배치. 이슈 재현 코드와 일치합니다.

## 3. 먼저 이슈에 댓글

#2659 를 닫을 때 메인테이너가 "순서는 보장 못 한다" 고 했습니다. 옵션이 무엇을 약속하는지 먼저 적고 반응을 봅니다.

> I traced this: the physical optimizer round-robins the single-partition source over
> `target_partitions` streams, and `write_streams` drains them concurrently into one writer
> channel, so batches interleave there. Everything after the channel is one sequential writer.
> An opt-in `preserve_order` that plans with `enable_round_robin_repartition` /
> `repartition_file_scans` off keeps a `RecordBatchReader` or `MemTable` input in order, at the
> cost of the scan-side parallelism only. I have a prototype with Rust and Python tests; happy
> to open a PR if that shape works for you, or to expose it differently if you'd rather.

## 4. Rust: `WriteBuilder` 에 옵션 추가

`crates/core/src/operations/write/mod.rs`

필드와 기본값:

```rust
    /// Keep the input's row order in the written files, see [`with_preserve_order`](Self::with_preserve_order)
    preserve_order: bool,
```

```rust
            safe_cast: false,
            preserve_order: false,
```

빌더 메서드 (`with_cast_safety` 다음):

```rust
    /// Write the rows in the order the input produces them.
    ///
    /// By default the input is spread over `target_partitions` streams that are
    /// drained concurrently, so rows of a multi-batch input land in the files in
    /// an arbitrary order. With `preserve_order` the planner keeps the input's
    /// partitioning (a single stream for a `RecordBatchReader` or a `MemTable`),
    /// so a pre-sorted input stays sorted inside each file, at the cost of the
    /// scan-side parallelism. Inputs with more than one partition keep their
    /// per-partition order only.
    pub fn with_preserve_order(mut self, preserve_order: bool) -> Self {
        self.preserve_order = preserve_order;
        self
    }
```

핵심 로직. `impl std::future::IntoFuture for WriteBuilder` 바로 위에 두고, import 에
`use datafusion::execution::session_state::{SessionState, SessionStateBuilder};` 를 추가합니다.

```rust
/// The physical optimizer splits a single-partition source into `target_partitions`
/// round-robin streams (and a `MemTable` by its batches), which the writer drains
/// concurrently; that is what reorders the rows. Turning both rules off keeps the
/// input's partitioning, so a single-stream input is written as it arrives.
fn without_repartitioning(state: SessionState) -> SessionState {
    let mut config = state.config().clone();
    let optimizer = &mut config.options_mut().optimizer;
    optimizer.enable_round_robin_repartition = false;
    optimizer.repartition_file_scans = false;
    SessionStateBuilder::new_from_existing(state)
        .with_config(config)
        .build()
}
```

`into_future` 안, `resolve_session_state(...)?;` 바로 다음:

```rust
                    let session = if this.preserve_order {
                        without_repartitioning(session)
                    } else {
                        session
                    };
```

사용자가 넘긴 session 은 복제본(`resolve_session_state` 가 `state.clone()` 을 돌려줌)이라 원본은 건드리지 않습니다.
`SessionStateBuilder::new_from_existing` 은 `merge/mod.rs:928` 에서 이미 같은 용도로 씁니다.

## 5. Rust 테스트

같은 파일 `mod tests` 끝에. 읽어 올 때 DataFusion scan 을 쓰면 또 파티션이 갈리므로 parquet 파일을 직접 읽습니다.
`target_partitions` 를 4 로 고정해서 코어 수에 따라 결과가 달라지지 않게 합니다. session 은 `create_session()` 에서 출발해야 합니다
(맨 `SessionContext::new()` 는 delta 의 query planner 가 없어 `MetricObserver` 노드를 못 풀고 planning 에러가 납니다).

```rust
    /// Writes `n_batches` batches of ascending `k` values and returns `k` as it is
    /// laid out in the written data files (one file per add action, in log order).
    async fn write_ascending_and_read_back(
        n_batches: usize,
        preserve_order: bool,
    ) -> TestResult<Vec<i64>> {
        use object_store::ObjectStoreExt;
        use object_store::path::Path;
        use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

        let schema = Arc::new(ArrowSchema::new(vec![Field::new(
            "k",
            DataType::Int64,
            false,
        )]));
        let batch_rows = 1024i64;
        let batches = (0..n_batches as i64).map(|i| {
            let start = i * batch_rows;
            RecordBatch::try_new(
                schema.clone(),
                vec![Arc::new(Int64Array::from_iter_values(
                    start..start + batch_rows,
                ))],
            )
            .unwrap()
        });

        // A fixed partition count so the test does not depend on the host's cores.
        let base = create_session().state();
        let mut config = base.config().clone();
        config.options_mut().execution.target_partitions = 4;
        let state = SessionStateBuilder::new_from_existing(base)
            .with_config(config)
            .build();
        let table = DeltaTable::new_in_memory()
            .write(batches)
            .with_session_state(Arc::new(state))
            .with_preserve_order(preserve_order)
            .await?;

        let log_store = table.log_store();
        let store = log_store.object_store();
        let files: Vec<_> = table
            .snapshot()?
            .snapshot()
            .file_views(&log_store, None)
            .try_collect()
            .await?;
        let mut read_back = Vec::new();
        for file in files.iter().rev() {
            let bytes = store
                .get(&Path::from(file.path().as_ref()))
                .await?
                .bytes()
                .await?;
            for batch in ParquetRecordBatchReaderBuilder::try_new(bytes)?.build()? {
                let batch = batch?;
                let col = batch
                    .column(0)
                    .as_any()
                    .downcast_ref::<Int64Array>()
                    .unwrap();
                read_back.extend(col.values().iter().copied());
            }
        }
        Ok(read_back)
    }

    #[tokio::test]
    async fn test_write_preserve_order_keeps_input_order() -> TestResult {
        let n_batches = 64;
        let read_back = write_ascending_and_read_back(n_batches, true).await?;
        assert_eq!(read_back.len(), n_batches * 1024);
        assert!(
            read_back.windows(2).all(|w| w[0] < w[1]),
            "rows must be written in input order"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_write_default_reorders_multi_batch_input() -> TestResult {
        // Documents the behaviour preserve_order opts out of: the planner spreads the
        // input over target_partitions streams, and the writer drains them concurrently.
        let n_batches = 64;
        let read_back = write_ascending_and_read_back(n_batches, false).await?;
        assert_eq!(read_back.len(), n_batches * 1024);
        let breaks = read_back.windows(2).filter(|w| w[0] > w[1]).count();
        println!(
            "default write: {breaks} order breaks in {} rows",
            read_back.len()
        );
        assert!(
            breaks > 0,
            "expected the default write to reorder the batches"
        );
        Ok(())
    }
```

```
cargo test -p deltalake-core --features datafusion --lib preserve_order -- --nocapture
```

main `21f4376c` 에서 돌린 결과 (2026-10-21, 코어 수와 무관하게 `target_partitions = 4`):

```
test operations::write::tests::test_write_preserve_order_keeps_input_order ... ok
default write: 15 order breaks in 65536 rows
test operations::write::tests::test_write_default_reorders_multi_batch_input ... ok
```

`operations::write` 모듈 테스트 93 개 전부 통과, `cargo fmt`, `cargo clippy --tests` 경고 없음.

두 번째 테스트는 "기본값은 섞인다" 를 고정하는 것이라 리뷰어가 flaky 를 걱정하면 빼도 됩니다. 첫 번째만 있어도 PR 은 성립합니다.

## 6. Python 배관

`python/src/lib.rs`: `RawDeltaTable::write` 와 모듈 함수 `write_to_deltalake` 양쪽의 `#[pyo3(signature = (...))]` 끝에
`preserve_order=false`, 파라미터에 `preserve_order: bool`, 그리고

```rust
            .with_save_mode(save_mode)
            .with_preserve_order(preserve_order);
```

`write_to_deltalake` 가 `raw_table.write(...)` 를 부르는 인자 목록 끝에도 `preserve_order` 추가.

`python/deltalake/_internal.pyi`: `write_to_deltalake(...)` 와 `RawDeltaTable.write(...)` 스텁에 `preserve_order: bool`.

`python/deltalake/writer/writer.py`: `@overload` 두 개와 본체 시그니처에 `preserve_order: bool = False`, docstring 한 단락,
`table._table.write(...)` 와 `write_deltalake_rust(...)` 호출에 `preserve_order=preserve_order`.

docstring:

```
        preserve_order: Write the rows in the order the input produces them. By default a
            multi-batch input (a RecordBatchReader, or a table with several chunks) is spread
            over several streams that are written concurrently, so its row order is not kept.
            Set to True when the input is pre-sorted (for example for clustering); it disables
            the scan-side parallelism of the write.
```

## 7. Python 테스트

`python/tests/test_writer.py`, `test_write_recordbatchreader` 다음. pyarrow 를 쓰므로 `@pytest.mark.pyarrow` 가 필요합니다
(CI 의 "Test without pyarrow" 스텝이 마커 없는 pyarrow 테스트를 그대로 돌려 실패시킵니다).

```python
@pytest.mark.pyarrow
def test_write_preserve_order_keeps_streamed_input_order(tmp_path: pathlib.Path):
    from itertools import pairwise

    import pyarrow as pa
    import pyarrow.parquet as pq

    schema = pa.schema([("k", pa.int64())])
    n, batch_rows = 256 * 1024, 4096

    def batches():
        for start in range(0, n, batch_rows):
            yield pa.RecordBatch.from_arrays(
                [pa.array(range(start, start + batch_rows), pa.int64())], schema=schema
            )

    write_deltalake(
        tmp_path,
        pa.RecordBatchReader.from_batches(schema, batches()),
        mode="overwrite",
        preserve_order=True,
    )
    dt = DeltaTable(tmp_path)
    k = []
    for uri in sorted(dt.file_uris()):
        k.extend(pq.read_table(uri).column("k").to_pylist())
    assert len(k) == n
    assert all(a < b for a, b in pairwise(k))
```

```
cd python && make develop && uv run --no-sync pytest tests/test_writer.py -k preserve_order
```

`make develop`(dev 프로파일) 후 통과. 이슈의 재현 코드(4M 행, 8192행 배치) 를 같은 빌드로 돌리면:

```
preserve_order=False: 47 breaks in 4194304 rows, 1 file(s)
preserve_order=True: 0 breaks in 4194304 rows, 1 file(s)
```

`ruff check`, `ruff format --check`, `mypy deltalake/writer/writer.py` 통과. `zip(k, k[1:])` 는 ruff RUF007 에 걸리니 `itertools.pairwise` 를 씁니다.

## 8. 마무리

- `cargo fmt --all`, `cargo clippy -p deltalake-core --features datafusion --tests`
- `git commit -s`, 제목 `feat: add preserve_order to the write operation`
- PR 본문에 직접 돌린 것(위 두 테스트, 이슈 재현 코드) 을 적을 것. AI 로만 만든 PR 은 닫힐 수 있음.
