# icebergCompatV2 writer 지원 계획 (#3299 의 C 안)

- 대상: `delta.enableIcebergCompatV2 = true` 인 테이블을 delta-rs 가 **스펙대로 쓰게** 만든다. Iceberg 메타데이터 생성은 범위 밖이다(스펙도 "이 기능은 변환을 구현하지 않는다" 고 적어 둠).
- 근거 스펙: `delta-io/delta` master `PROTOCOL.md` 의 "Iceberg Compatibility V2", "Materialize Partition Columns", `delta.writePartitionColumnsToParquet` 속성 설명(2026-10-19 확인).
- 검증: upstream main `f67f1210`(2026-10-19) 에 아래 3 개 PR 을 한 번에 적용해 돌려 봤다. 적용한 diff 전체는 `prototype.diff`, 결과는 7 절.
- V1 은 하지 않는다(4 절).

## 0. 한 줄 결론

코드량은 작다(테스트 빼고 약 270 줄). 어려운 부분은 두 가지다.
파티션 컬럼을 parquet 에 쓰는 변경이 write 경로 전부에 걸치는 것, 그리고 kernel 이 V2 검증을 아직 안 해서(`iceberg_compat/mod.rs` 에 `TODO: Add V1, V2`) delta-rs 가 검증을 직접 들고 있어야 하는 것이다.
그래서 PR 을 셋으로 나누고, kernel 과 무관한 PR 1·2 를 먼저 올린다.

## 1. 스펙 요구사항과 delta-rs 현황

| 스펙 (Writer Requirements for IcebergCompatV2) | delta-rs | 할 일 |
|---|---|---|
| Column Mapping `name`/`id` 필수 | 지원함 (`datafusion` feature) | 속성 검증만 |
| array `element`, map `key`/`value` 에 32bit id 를 주고 `delta.columnMapping.nested.ids` 에 저장 | **kernel 이 이미 구현.** `assign_column_mapping_metadata(schema, max_id, assign_nested_field_ids)` 의 셋째 인자. `create.rs` 가 `false` 로 호출 중 | `true` 로 호출 (PR 2) |
| 그 id 를 parquet `SchemaElement.field_id` 에도 기록 | `column_mapping.rs` 가 struct 필드에는 `PARQUET:field_id` 를 붙이지만 element/key/value 에는 안 붙임 | 붙이기 (PR 2) |
| V1 과 동시 활성 금지 | 검사 없음 | 속성 검증 (PR 3) |
| Deletion Vectors 비활성 | delta-rs 는 DV 를 **쓰지 않음**(읽기만). 속성 충돌만 막으면 됨 | 속성 검증 (PR 3) |
| 파티션 값을 parquet 에 materialize | 모든 writer 가 파티션 컬럼을 떼고 씀 (`arrow_schema_without_partitions`, `record_batch_without_partitions`) | 떼는 지점 3 곳 수정 (PR 1) |
| 파티션 컬럼 stats 금지 (2508·2558 줄, should) | `stats_from_metadata` 가 이미 `partition_values` 에 있는 컬럼을 건너뜀 | 없음. 테스트로만 확인 |
| 모든 AddFile 에 `numRecords` | `create_add` 가 항상 씀 | 없음 |
| timestamp 는 int64 | arrow parquet writer 기본값. INT96 쓰는 코드 없음 | 없음 |
| 타입 allow-list (byte~struct 16 종) | 검사 없음. Variant·Void·(nanos) 가 걸림 | create 시 검사 (PR 3). CM 테이블은 `add_column`·schema evolution·mode 변경이 이미 막혀 있어 create 한 곳이면 충분 |
| 파티션 spec 이름 바꾸는 replace 금지 | CM 테이블에서 schema 변경이 막혀 있음 | 없음 |
| Type Widening 제약 | delta-rs 는 typeWidening writer 미지원 → 조건부 요구사항이라 해당 없음 | 없음 |
| `writerFeatures` 에 `icebergCompatV2`, Writer Version 7 | `protocol.rs` INSTANCE 지원 목록에 없음 | 추가 (PR 3) |

kernel 쪽: `TableFeature::IcebergCompatV2` 와 `table_properties.enable_iceberg_compat_v2` 는 있고, `is_feature_enabled` 가 그대로 동작한다. kernel 의 `validate_iceberg_compat_if_needed` 는 V3 만 검사하므로 V2 테이블을 막지 않는다. delta-rs 는 kernel 의 write support 검사를 호출하지 않으므로 지원 여부는 `protocol.rs` 의 INSTANCE 목록이 결정한다.

## 2. PR 1 — `materializePartitionColumns` writer feature 지원 ★★★

스펙에 별도 feature 로 정의돼 있고 "IcebergCompat 요구사항의 부분집합" 이라고 적혀 있다. Iceberg 얘기 없이 제안할 수 있어 **가장 먼저, 가장 받아들여지기 쉬운 PR** 이다.

판정 helper 하나를 두고 모든 write 경로가 공유한다.

```rust
// crates/core/src/datafile/properties.rs
pub(crate) fn materialize_partition_columns(config: &TableConfiguration) -> bool {
    config.is_feature_supported(&TableFeature::MaterializePartitionColumns)
        || config.is_feature_enabled(&TableFeature::IcebergCompatV1)
        || config.is_feature_enabled(&TableFeature::IcebergCompatV2)
        || config.table_properties().unknown_properties
            .get(TableProperty::WritePartitionColumnsToParquet.as_ref())
            .is_some_and(|v| v.eq_ignore_ascii_case("true"))
}
```

| 파일 | 변경 |
|---|---|
| `table/config.rs` | `TableProperty::WritePartitionColumnsToParquet` 추가 |
| `datafile/writer/dataset.rs` | `WriterConfig.materialize_partition_columns` 플래그. `file_schema()` 와 `write_partition` 이 플래그가 켜지면 파티션 컬럼을 떼지 않음 |
| `operations/write/execution.rs` | `WriteSinkConfig::new` 에서 helper 로 플래그 계산, `WriterConfig` 3 곳(일반·CDC 본 파일·CDC 변경 파일)에 전달. write/merge/update/delete 가 이 경로를 지남 |
| `operations/optimize.rs` | `file_schema` 계산에서 플래그가 켜지면 `arrow_schema_without_partitions` 를 건너뜀. 스캔이 로그의 파티션 값을 컬럼으로 내놓으므로 그대로 parquet 에 들어감 |
| `kernel/transaction/protocol.rs` | INSTANCE writer 목록에 `MaterializePartitionColumns` (datafusion cfg 안) |
| `writer/mod.rs` | legacy `RecordBatchWriter`/`JsonWriter` 는 파티션 컬럼을 무조건 떼므로 `ensure_legacy_writer_supports_table` 에서 거부 |

주의:
- CM 테이블에서는 파티션 컬럼도 물리 이름·field id 로 써야 한다(스펙 2506 줄). `ColumnMappingExec` 가 sink 앞에서 전체 배치를 물리 이름으로 바꾸므로 파티션 컬럼을 남겨 두기만 하면 된다. 프로토타입 테스트(3 절)에서 확인함.
- 읽기 쪽은 손대지 않는다. 스캔은 파티션 값을 로그에서 읽고, parquet 에 같은 컬럼이 있어도 중복되지 않았다(테스트에서 읽은 스키마가 3 컬럼).
- stats 제외는 이미 되어 있어 코드가 없다. 테스트로만 지킨다.

테스트(`operations/write/mod.rs`): 파티션 테이블에 쓰기 → feature 추가 → append → optimize. 모든 데이터 파일 컬럼이 `["id","value","modified"]`, stats 에 `numRecords` 는 있고 `modified` 는 없음, 읽으면 22 행 3 컬럼. 속성 `delta.writePartitionColumnsToParquet=true` 만으로도 같은 결과.

## 3. PR 2 — 중첩 field id ★★

`create.rs` 에서 V2 가 켜지면 kernel 에 nested id 할당을 맡긴다.

```rust
let nested_ids = configuration.get(TableProperty::EnableIcebergCompatV2.as_ref())
    .is_some_and(|v| v.eq_ignore_ascii_case("true"));
let mapped = assign_column_mapping_metadata(&schema, &mut max_id, nested_ids)?;
```

`column_mapping.rs` 에서는 필드의 `delta.columnMapping.nested.ids`(JSON 객체) 를 읽어 재귀하면서 경로를 만든다. 키 규칙은 스펙 예시대로 `<물리이름>.element`, `<물리이름>.key`, `<물리이름>.value.element` 이다. array 안의 struct 는 StructField 라서 자기 metadata 를 따로 가지므로 경로가 그 필드의 물리 이름에서 다시 시작한다.

```rust
struct NestedFieldIds { ids: HashMap<String, i64>, path: String }
// physical_field 에서 NestedFieldIds::of(kernel_field, mode) 로 만들고
// rebuild_element / map key / map value 에서 child("element"|"key"|"value") 로 내려가며
// attach() 가 PARQUET:field_id 를 붙인다
```

arrow-rs parquet writer 는 list element·map key/value Field 의 `PARQUET:field_id` metadata 도 `field_id` 로 써 준다. 테스트에서 parquet 스키마를 다시 읽어 확인했다.

이 PR 도 `TableProperty::EnableIcebergCompatV2` variant 가 있어야 하므로(#3299 의 A 안), 그 variant 3 개를 여기 포함한다.

테스트: `arr: array<int>`, `m: map<string, array<int>>`, `s: struct{inner: array<int>}`, `p: string`(파티션) 테이블을 CM name + V2 로 만들고 한 파일을 쓴 뒤 parquet 스키마의 id 를 metadata 와 대조. `maxColumnId` 는 컬럼 5 + nested 5 = 10.

## 4. PR 3 — `icebergCompatV2` 활성화 ★★ (PR 1·2 의존)

| 파일 | 변경 |
|---|---|
| `kernel/models/actions.rs` `apply_properties_to_protocol` | V2 가 `true` 면: CM 이 속성에 있거나 protocol 에 이미 있는지 검사, V1·DV 와 동시면 에러, `min_writer_version = 7`, `writerFeatures += icebergCompatV2`. CM 검사보다 **앞에** 둬야 CM 분기가 writer 7 을 보고 feature 로 넣는다 |
| 같은 파일 `move_table_properties_into_features` | `"delta.enableIcebergCompatV2"` → `IcebergCompatV2` |
| 같은 파일 `TableFeatures` | `IcebergCompatV2` variant + `"icebergCompatV2"` 문자열 (`add_feature` 용) |
| `kernel/transaction/protocol.rs` | INSTANCE writer 목록에 `IcebergCompatV2` |
| `operations/create.rs` | `ensure_iceberg_compat_v2_types`: Void·Variant·(nanos timestamp) 거부, 중첩 타입 재귀 |
| `operations/set_tbl_properties.rs` | V2 를 나중에 켜는 것은 거부. nested id 는 create 때만 할당하기 때문 |

V1 은 넣지 않는다. V1 은 Map/Array 자체를 금지하고, DV 가 protocol 에 아예 없어야 하고, 파티션 컬럼을 데이터 컬럼 뒤에 둬야 한다. Spark 도 V2 를 권장한다. `delta.enableIcebergCompatV1` 키는 파싱만 되고(A 안) `true` 면 V2 와 충돌 에러만 낸다. V1 단독 `true` 는 현재 아무 효과가 없는데, PR 3 에서 "not supported" 에러로 바꾸는 편이 낫다(미구현).

테스트: 위 PR 2 테이블에서 protocol 이 writer 7 + `icebergCompatV2` + `columnMapping` 인지, `is_feature_enabled` 가 true 인지, `PROTOCOL.can_write_to` 가 통과하는지, 데이터 파일에 파티션 컬럼 `p` 가 물리 이름으로 들어가고 stats 에는 없는지. 거부 케이스 4 개(CM 없음, +DV, +V1, Variant 타입).

## 5. 범위 밖·한계 (PR 본문에 적을 것)

- Iceberg 메타데이터 생성은 하지 않는다. 사용자가 Iceberg 로 읽으려면 외부 도구(Spark UniForm 등)가 필요하다.
- V1 writer 미지원.
- 기존 CM 테이블에 V2 를 나중에 켜는 것은 막는다(nested id 없음). 풀려면 schema metadata 를 다시 쓰는 별도 작업이 필요하다.
- legacy `RecordBatchWriter`/`JsonWriter` 는 materialize 테이블을 거부한다. 지원하려면 `writer/record_batch.rs:308` 근처 `WriteWindow` 에 같은 플래그가 필요하다.
- kernel 이 나중에 V2 validator 를 추가하면 PR 3 의 검증과 겹친다. PR 3 은 얇게 유지하고 본문에 적어 둔다.

## 6. 진행 순서

1. #3299 에 댓글: "writer 요구사항을 구현해서 실제 지원으로 가면 받겠는가, 그렇다면 `materializePartitionColumns` 지원부터 별도 PR 로 올려도 되는가" 를 묻는다. 이 계획의 1 절 표를 붙이면 된다.
2. PR 1 → PR 2 → PR 3 순서로 올린다. PR 1 은 단독으로도 가치가 있다.
3. 각 PR 에 "직접 검증한 것"(7 절) 을 적는다. CONTRIBUTING 의 AI 정책 때문이다.

## 7. 프로토타입 검증 결과 (2026-10-19, upstream main `f67f1210`)

`prototype.diff` 를 적용한 상태에서:

- 새 테스트 4 개 통과: `test_materialize_partition_columns_keeps_partition_columns_in_data_files`, `test_write_partition_columns_to_parquet_property`, `test_iceberg_compat_v2_writes_nested_field_ids`, `test_iceberg_compat_v2_rejects_unsupported_configuration` (모두 `crates/core/src/operations/write/mod.rs`).
- 기존 테스트: `cargo test -p deltalake-core --features datafusion --lib` 1358 통과(2 ignored), `--test it_datafusion` 167 통과(8 ignored), `--test it` 49 통과.
- `cargo fmt`, `cargo clippy -p deltalake-core --features datafusion --tests` 에서 수정 파일 경고 없음.
- Iceberg 리더로 실제로 읽어 보지는 않았다(pyiceberg/Spark 환경 없음). 스펙의 각 항목을 parquet 메타데이터와 로그로 확인한 것까지다.

## 8. 실행

```bash
# upstream main 에 적용해 보기
git fetch upstream main && git checkout -b jx2lee/feat/iceberg-compat-v2 upstream/main
git apply contribution/iceberg-compat-v2-writer/prototype.diff
cargo fmt --all
cargo clippy -p deltalake-core --features datafusion --tests
cargo test -p deltalake-core --features datafusion --lib -- operations::write::tests::test_materialize operations::write::tests::test_iceberg_compat_v2 operations::write::tests::test_write_partition_columns
```

PR 별로 나눌 때는 diff 를 파일 단위로 갈라 쓴다. PR 1: `properties.rs`, `dataset.rs`, `execution.rs`, `optimize.rs`, `writer/mod.rs`, `protocol.rs` 의 `MaterializePartitionColumns`, `config.rs` 의 `WritePartitionColumnsToParquet`. PR 2: `column_mapping.rs`, `create.rs` 의 nested id, `config.rs` 의 Iceberg variant 3 개. PR 3: 나머지.
