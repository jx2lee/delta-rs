# 수정 가이드: metadata_size_hint 가 ParquetSource 에 전달되지 않음 (#4864)

난이도: ★☆☆☆☆ (Rust 입문자용, 변경 10줄 이내)

## 1. 문제를 한 줄로

DataFusion 의 `ParquetSource` 는 "옵션 묶음(`TableParquetOptions`)" 과 "푸터 선읽기 힌트(`metadata_size_hint`)" 를
**서로 다른 필드**에 들고 있습니다. delta-rs 는 옵션 묶음만 넘기고 힌트 setter 를 호출하지 않아서,
사용자가 세션에 설정한 힌트가 버려집니다.

## 2. 먼저 읽어 둘 코드

| 파일 | 줄 | 무엇 |
|---|---|---|
| `crates/core/src/delta_datafusion/table_provider/next/scan/mod.rs` | 947 | `pq_options` 생성 (세션의 parquet 옵션 복사) |
| 같은 파일 | 977-979 | `ParquetSource::new(...).with_table_parquet_options(pq_options.clone())` |
| `crates/core/src/operations/load_cdf.rs` | 611-623 | CDF 용 `ParquetSource` 3개 (cdc/add/remove) |
| `~/.cargo/registry/src/*/datafusion-datasource-parquet-55.1.0/src/source.rs` | 347-362 | 두 setter 가 다른 필드를 쓰는 것을 직접 확인 |
| 같은 crate `src/file_format.rs` | 490-513 | DataFusion 이 자기 스캔에서 힌트를 적용하는 방식 (그대로 따라하면 됨) |

Rust 개념 메모:
- `let mut x = X::new().with_a(..)` 뒤에 `x = x.with_b(..)` 처럼 **builder 패턴**은 자기 자신을 소비하고 돌려줍니다. `mut` 가 붙어 있어야 다시 대입할 수 있습니다.
- `Option<usize>` 는 값이 있을 수도(Some) 없을 수도(None) 있는 타입. `if let Some(v) = opt { ... }` 로 꺼냅니다.

## 3. 수정 (diff)

### 3-1. 메인 스캔 (`next/scan/mod.rs` 977 근처)

```rust
        let mut file_source = ParquetSource::new(table_schema)
            .with_table_parquet_options(pq_options.clone())
            .with_parquet_file_reader_factory(reader_factory);
+       if let Some(metadata_size_hint) = pq_options.global.metadata_size_hint {
+           file_source = file_source.with_metadata_size_hint(metadata_size_hint);
+       }
```

`file_source` 는 이미 `let mut` 이므로 추가 선언 없이 대입만 하면 됩니다.

### 3-2. CDF (`load_cdf.rs` 615 근처)

세 개의 source 가 같은 패턴이라 작은 클로저로 묶으면 중복이 없습니다.

```rust
        let parquet_options = TableParquetOptions {
            global: session.config().options().execution.parquet.clone(),
            ..Default::default()
        };
+       let metadata_size_hint = parquet_options.global.metadata_size_hint;
+       let apply_hint = |source: ParquetSource| match metadata_size_hint {
+           Some(hint) => source.with_metadata_size_hint(hint),
+           None => source,
+       };

-       let mut cdc_source = ParquetSource::new(cdc_table_schema)
-           .with_table_parquet_options(parquet_options.clone())
-           .with_parquet_file_reader_factory(self.parquet_metadata_cache.clone());
+       let mut cdc_source = apply_hint(
+           ParquetSource::new(cdc_table_schema)
+               .with_table_parquet_options(parquet_options.clone())
+               .with_parquet_file_reader_factory(self.parquet_metadata_cache.clone()),
+       );
        // add_source, remove_source 도 동일하게 감싼다
```

클로저가 어색하면 세 군데에 `if let Some(..)` 블록을 각각 넣어도 됩니다. 리뷰어는 어느 쪽이든 받아줄 가능성이 높습니다.

## 4. 테스트 추가 (선택이지만 권장)

`DataFusion` 의 `metadata_size_hint` 필드는 `pub(crate)` 라 바깥에서 직접 검사할 수 없습니다.
대신 "힌트가 적용되면 object store 의 `get_range` 호출 횟수가 줄어든다" 로 검증할 수 있습니다.

1. `crates/core/tests/it_datafusion/` 에서 이미 쓰는 **호출 추적용 ObjectStore 래퍼** 패턴을 찾습니다.
   `command_optimize.rs` 의 `OperationTrackingLogStore` 처럼 `get_range` 호출을 `Arc<Mutex<Vec<..>>>` 에 기록하는 래퍼를 만듭니다.
2. 세션을 두 개 만듭니다.
   ```rust
   let mut cfg = SessionConfig::new();
   cfg.options_mut().execution.parquet.metadata_size_hint = Some(1024 * 1024);
   ```
3. 같은 테이블을 두 세션으로 `read_table(...).collect()` 하고, 힌트가 있는 쪽의 `get_range` 호출 수가 적은지 `assert!` 합니다.
   (힌트 없음: 파일당 8바이트 + 푸터 = 2번, 힌트 있음: 파일당 1번)

간단히 가려면 테스트 없이 올리고 PR 본문에 "DataFusion 의 `ParquetFormat::create_physical_plan` 과 같은 방식" 임을 적어도 됩니다.
이 저장소는 codecov 봇이 patch coverage 를 코멘트하지만 작은 fix 에 필수는 아닙니다.

## 5. 실행 순서

```bash
cargo fmt --all
cargo clippy -p deltalake-core --features datafusion --tests
cargo test -p deltalake-core --features datafusion load_cdf    # CDF 회귀 확인
git checkout -b fix/parquet-metadata-size-hint
git commit -s -m "fix: propagate parquet metadata_size_hint to delta scans"
```

## 6. 주의

- CONTRIBUTING.md 에 **AI 로만 만든 PR 은 닫힐 수 있다**고 명시돼 있습니다(최근 #4437 도 그 이유로 닫힘).
  PR 본문에 "DataFusion 소스 어디를 읽고 확인했는지" 를 직접 쓰면 신뢰를 얻습니다.
- 이슈에 먼저 `take` 라고 댓글을 달면 자동 할당됩니다.
