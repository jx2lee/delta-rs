# 수정 가이드: 중첩 스키마 override 가 Parquet 읽기에서 다시 넓어짐 (#4678)

난이도: ★★★★☆ (kernel 스캔 플래닝 이해 필요. 두 번 닫힌 PR 이 있으니 리뷰 포인트를 미리 맞춰야 함)

## 1. 문제를 한 줄로

사용자가 "`s` 구조체에서 `l` 만 읽겠다" 고 override 하면 논리 스키마는 좁아지는데,
kernel 스캔은 **루트 컬럼 이름만** 으로 projection 해서 `s` 전체(`l`,`b`)를 다시 읽습니다.

## 2. 코드 읽기 (`crates/core/src/delta_datafusion/table_provider/next/scan/plan.rs`)

| 줄 | 무엇 |
|---|---|
| 49-69 | `ProjectedScanContract`: `scan_schema`(논리, 좁혀짐), `kernel_projection`(루트 인덱스) |
| 242-265 | `KernelScanPlan::try_new` → `try_new_with_contract` |
| 306-317 | **버그 지점**: `kernel_logical_schema.project(&names)` 루트 이름 projection 후 `with_schema` |
| 318-321 | `parquet_read_schema = config.physical_arrow_schema(table_config, scan.physical_schema())` |
| 388-400 | `physical_arrow_schema`: 논리→물리 "타입" 변환만 함. 파티션 제거·column mapping 이름 변환은 안 함 (리뷰어 지적) |
| 1047-1110 | 기존 테스트 `test_scan_schema_contract` (루트 override 만 다룸) |

kernel 쪽 (`~/.cargo/git/checkouts/delta-kernel-rs-*/8ba063f/kernel/src/scan/`):

| 파일:줄 | 무엇 |
|---|---|
| `mod.rs:254-264` | `ScanBuilder::with_schema(logical_read_schema)`: "읽을 컬럼" 스키마 |
| `state_info.rs:267-` | 논리 필드 하나씩 → 물리 필드 + transform 생성. **논리 스키마가 좁으면 물리 스키마도 좁아짐** |

Rust 개념 메모:
- `SchemaRef` = `Arc<Schema>`. `Arc` 는 참조 카운트 포인터라 `.clone()` 이 싸고 내용은 공유됩니다.
- Arrow `Schema` ↔ kernel `StructType` 변환은 `try_into_kernel()` / `try_into_arrow()` (`delta_kernel::engine::arrow_conversion::{TryIntoKernel, TryIntoArrow}`) 로 합니다. 이미 `plan.rs` 에서 `try_into_arrow()` 를 쓰고 있습니다.

## 3. 두 가지 접근

### A. 이슈가 제안한 최소 수정 (리뷰어가 거절한 방향)

`parquet_read_schema` 만 `contract.scan_schema` 로 바꾸는 것. #4685 가 이 방식이었고 ethan-tyler 가
"논리 vs 물리 스키마 혼동, 파티션/column mapping 미반영, kernel transform 과 어긋남" 으로 막았습니다. 따라 하지 마세요.

### B. kernel 에 넘기는 논리 스키마 자체를 중첩까지 좁히기 (권장)

306-317 줄에서 루트 이름 projection 대신, `contract.scan_schema`(Arrow) 의 각 필드를 테이블 논리 스키마(kernel `StructType`) 에서
**이름으로 찾아 중첩까지 내려가며** 골라낸 kernel `StructType` 을 만들어 `with_schema` 에 넘깁니다.
kernel 이 그 스키마로 물리 스키마와 transform 을 함께 만들기 때문에 318-321 줄은 그대로 두어도 `parquet_read_schema` 가 좁아집니다.

```rust
/// table 의 kernel 필드에서 `wanted`(arrow) 에 있는 자식만 남긴다. metadata(column mapping id 등)는 원본 것을 유지.
fn narrow_kernel_struct(table: &StructType, wanted: &arrow_schema::Fields) -> DeltaResult<StructType> {
    let mut out = Vec::with_capacity(wanted.len());
    for w in wanted.iter() {
        let f = table.field(w.name()).ok_or_else(|| DeltaTableError::Generic(format!("field {} not found", w.name())))?;
        let narrowed = match (f.data_type(), w.data_type()) {
            (DataType::Struct(inner), arrow_schema::DataType::Struct(wf)) => {
                StructField::new(f.name(), DataType::Struct(Box::new(narrow_kernel_struct(inner, wf)?)), f.is_nullable())
                    .with_metadata(f.metadata().clone())   // 실제 메서드 이름은 kernel StructField 문서로 확인
            }
            _ => f.clone(),
        };
        out.push(narrowed);
    }
    Ok(StructType::new(out))
}
```

그리고 호출부:

```rust
-            let kernel_scan_schema = kernel_logical_schema
-                .project(&kernel_projection_names)
-                .map_err(crate::DeltaTableError::from)?;
+            let kernel_scan_schema = Arc::new(narrow_kernel_struct(
+                kernel_logical_schema,
+                contract.scan_schema.fields(),
+            )?);
```

주의할 점:
1. 리스트/맵 안의 struct(`list<struct<..>>`) 까지 재귀할지 정합니다. 처음엔 struct 만 다루고 나머지는 `f.clone()` 으로 넓게 두면 안전합니다("narrowing 이 안전하지 않으면 넓은 입력 유지" 가 리뷰어 요구).
2. `with_metadata` 로 column mapping 의 `delta.columnMapping.physicalName`/`id` 메타데이터를 반드시 유지해야 합니다. 안 그러면 column mapping 테이블에서 물리 이름을 못 찾습니다.
3. 필드 **순서**는 테이블 스키마 순서가 아니라 override 순서가 됩니다. #4729 리뷰 지적("transform 은 원래 순서를 기대")이 여기서도 문제인지 column-mapped 테이블 테스트로 확인하세요. 문제가 되면 `wanted` 를 순회하지 말고 `table` 을 순회하면서 `wanted` 에 있는 것만 남기는 식으로 바꾸면 순서가 유지됩니다.

## 4. 실험 순서 (코드를 쓰기 전에)

1. `plan.rs` 테스트 모듈 끝에 issue.md 의 재현 테스트를 붙이고 실패를 확인합니다. (2026-10-13 확인: 실패)
2. B 를 적용하고 같은 테스트가 통과하는지 봅니다.
3. **리뷰어가 요구한 end-to-end 테스트** 를 추가합니다.
   - `DeltaTableProvider` 로 `read_table(...).collect()` 해서 결과 행이 기대와 같은지.
   - Parquet 가 실제로 `s.l` 만 읽는지: `crates/core/tests/it_datafusion/` 의 호출 추적 ObjectStore 래퍼 패턴으로 `get_range` 요청 바이트 범위가 `s.b` 컬럼 청크를 포함하지 않는지, 또는 `DeltaScanExec` 의 `ParquetSource` file schema 를 `displayable` plan 문자열에서 확인.
   - 픽스처 두 개: 평범한 `table_with_null_stats_in_notnull_struct` + column mapping 이 켜진 `table_with_column_mapping`(파티션 포함) 으로 "파티션 컬럼이 parquet 스키마에 없고 물리 이름이 맞는지".
4. `cargo test -p deltalake-core --features datafusion next::scan` 전체와 Python `uv run pytest tests -k "schema or projection"` 으로 회귀 확인.

## 5. PR 올리기 전에

- 이슈에 댓글로 "B 방향으로 가겠다, #4685 리뷰 코멘트 4개를 이렇게 처리한다" 를 먼저 적으세요. rtyler 가 #4685 를 닫은 이유가 "babysit 비용" 이었으니, 리뷰어가 할 말을 미리 답해 두는 것이 핵심입니다.
- `@ethan-tyler` 가 이 영역 담당입니다.

## 6. 실행

```bash
cargo fmt --all
cargo clippy -p deltalake-core --features datafusion --tests
cargo test -p deltalake-core --features datafusion --lib next::scan
git commit -s -m "fix: keep nested schema overrides when building the kernel scan schema"
```
