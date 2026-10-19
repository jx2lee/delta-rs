# icebergCompatV2 프로토타입 — Spark UniForm + pyiceberg 통합 검증 (2026-10-19)

`plan.md` 의 프로토타입이 쓴 테이블을 **Delta Spark 가 Iceberg 메타데이터로 변환하고, pyiceberg 가 읽는 것**까지 돌려 본 기록이다.
delta-rs 는 Iceberg 메타데이터를 만들지 않으므로 이 경로가 "Iceberg 리더로 읽힌다" 를 확인할 수 있는 유일한 방법이다.
돌리면서 프로토타입에 없던 문제 둘을 찾아 고쳤고(3 절), 그 결과가 `prototype.diff` 에 반영돼 있다.

## 0. 한 줄 결론

- 프로토타입이 쓴 파일은 Spark 의 IcebergCompatV2 검사를 통과하고, Spark 가 만든 Iceberg 스키마의 field id 1~18 이 delta-rs 가 준 column mapping id·nested id 와 그대로 일치한다. 데이터 파일은 다시 쓰이지 않고 그 자리에서 manifest 에 등록됐다. pyiceberg 는 중첩 타입·decimal·timestamp·파티션 프루닝까지 정확히 읽었다.
- 찾은 문제: (1) delete/update/merge/optimize 가 다시 쓴 파일의 `ARROW:schema` 에 `string_view`·`dictionary` 가 들어가 pyarrow 기반 리더(pyiceberg)가 다른 파일과 합치지 못한다. **main 에도 있는 버그**다(PR 0 으로 분리). (2) Spark 가 파일마다 다는 `ICEBERG_COMPAT_VERSION` tag 가 없으면 `REORG ... UPGRADE UNIFORM` 이 delta-rs 파일을 전부 다시 쓴다(PR 3 에 포함).
- 범위 밖으로 확인된 것: column mapping 테이블에서 delta-rs `OPTIMIZE` 가 거부됨(기존 guard, V2 와 무관).

## 1. 환경

| 구성 | 버전 | 비고 |
|---|---|---|
| Python | 3.12 (`uv venv`) | `uv pip install delta-spark "pyiceberg[pyarrow,sqlite]"` |
| pyspark / delta-spark | 4.2.0 / 4.4.1 | `io.delta:delta-iceberg_2.13:4.4.1` 을 `configure_spark_with_delta_pip(extra_packages=...)` 로 추가. 안에 Iceberg 1.11.0 이 shaded 로 들어 있음 |
| pyiceberg / pyarrow | 0.12.0 / 26.0.0 | `StaticTable.from_metadata()` 로 metadata.json 직접 읽음 |
| Java | OpenJDK 21 | Spark 4.2 요구 |
| 메타스토어 | 내장 Derby(`enableHiveSupport`) | UniForm 은 카탈로그 테이블이어야 변환을 돌림. `CREATE TABLE uni USING DELTA LOCATION '<path>'` 로 등록 |
| delta-rs | upstream main `f67f1210` + `prototype.diff` | 테이블 생성·읽기·쓰기는 `integration/uniform_export.rs`, `integration/uniform_roundtrip.rs` (ignored 테스트, `UNIFORM_OUT` 환경변수) |

걸린 곳: `~/.m2` 에 pom 만 있고 jar 가 없는 의존성이 있으면 ivy 가 `local-m2-cache` 에서 "download failed" 로 죽는다. `integration/ivysettings.xml`(central 만 보는 설정)을 `spark.jars.ivySettings` 로 주면 된다.

## 2. 시나리오와 결과

테이블: `id long, name string, flag boolean, amount decimal(10,2), ratio double, d date, ts timestamp, arr array<int>, m map<string, array<int>>, s struct<inner: array<int>, note: string>, p string`(파티션). CM `name`, `delta.enableIcebergCompatV2=true`, `delta.universalFormat.enabledFormats=iceberg` 를 **create 때** 준다(#3299 의 사용자 코드와 같은 모양). delta-rs 가 두 번 써서 파일 3 개, 4 행(파티션 x·y).

| # | 단계 | 결과 |
|---|---|---|
| 1 | Spark 가 테이블 등록 후 `SELECT *` | 4 행 전부 정확. 중첩·decimal·timestamp·null 포함. `DESCRIBE DETAIL` 이 writer 7 + `[appendOnly, columnMapping, icebergCompatV2, invariants]` |
| 2 | Spark 의 첫 커밋(`ALTER TABLE SET TBLPROPERTIES`, 값 변경 없음) | Spark 의 IcebergCompatV2 검사(타입·CM·DV·stats) 통과. UniForm hook 이 `metadata/` 에 Iceberg v2 메타데이터 생성. `added-data-files 3, added-records 4`, 파일 경로는 delta-rs 가 쓴 그대로 |
| 3 | Iceberg 스키마 대조 | field id 1~13 = column mapping id, 14~18 = `delta.columnMapping.nested.ids`(list `element-id`, map `key-id`/`value-id`, 중첩 list 의 `element-id`). 파티션 spec `identity(13)`. `schema.name-mapping.default` 는 논리 이름 기준이라 parquet 의 물리 이름과는 안 맞음 → **리더가 parquet `field_id` 로 컬럼을 찾아야만 읽힌다**. 즉 PR 2 가 하는 일이 실제로 검증됨 |
| 4 | pyiceberg `StaticTable` 읽기 | 4 행 값 일치. `row_filter="p = 'y'"` → `[3, 4]`(manifest 파티션 값으로 프루닝), `id >= 3` + 컬럼 projection 정상 |
| 5 | Spark 가 `INSERT` 1 행 | 성공. Spark 파일은 `tags: {ICEBERG_COMPAT_VERSION: 2}` 를 달고 `ARROW:schema` 없음(parquet-mr) |
| 6 | delta-rs 가 그 테이블을 다시 읽고 씀 | Spark 가 쓴 행 포함 5 행 읽힘. append(v5)·`DELETE id = 2`(v6, 파티션 x 파일 재작성) 성공. **`OPTIMIZE` 는 `UnsupportedColumnMapping { mode: Write, operation: "OPTIMIZE" }` 로 거부**(4 절) |
| 7 | Spark 의 no-op 커밋으로 v4~v7 증분 변환 → pyiceberg | 처음엔 **실패**: `Unable to merge: Field name has incompatible types: string vs string_view`(3-1). 수정 후 `[1, 3, 4, 5, 6]` 값 일치 |
| 8 | `REORG TABLE uni APPLY (UPGRADE UNIFORM (ICEBERG_COMPAT_VERSION = 2))` | 처음엔 delta-rs 파일 4 개 전부 재작성(`numRemovedFiles 4`), Spark 파일만 유지(3-2). 수정 후 다시 쓸 파일이 없어 커밋조차 생기지 않음 |

Spark 가 v3 커밋에서 metaData 를 다시 썼는데, 모든 leaf 필드에 `delta.columnMapping.nested.ids: {}` 를 채워 넣었다(id 값은 그대로). delta-rs 는 빈 객체를 문제없이 읽고 그 뒤 append·delete 도 정상이었다.

## 3. 찾은 문제와 수정

### 3-1. 다시 쓴 파일의 `ARROW:schema` 에 in-memory 타입이 들어감 → PR 0 (Iceberg 와 무관, 별도 이슈)

- 증상: delete 가 재작성한 파일만 pyarrow 가 `name: string_view`, 파티션 컬럼 `dictionary<values=string, indices=uint16>` 로 복원한다. 첫 write 파일은 `string`. pyiceberg 는 파일별 결과를 `pa.concat_tables` 로 합치다 실패하고, 단일 파일이라도 `array_filter` 커널이 없어 필터가 깨진다.
- 원인: DataFusion 스캔이 `schema_force_view_types=true` 로 `Utf8View` 를 내고(파티션 컬럼은 dictionary), delete/update/merge/optimize 는 그 plan 스키마를 writer 스키마로 쓴다. arrow writer 가 그 스키마를 `ARROW:schema` footer 에 그대로 직렬화한다. parquet 물리 타입은 둘 다 `BYTE_ARRAY/UTF8` 이라 Spark·Java 리더는 영향이 없고, footer 를 믿는 pyarrow 계열만 깨진다.
- **main 에서 재현**: 프로토타입 없이 `f67f1210` 에서 파티션 테이블에 write → `delete("id = 2")` 하면 재작성 파일이 `name: string_view`. #4579 가 같은 증상인데 #4589 는 compression 만 고치고 닫혔다. pyarrow 가 최근 버전에서 `string_view` 를 읽을 수 있게 되면서 넘어간 듯하나, 파일을 합칠 때는 여전히 실패한다.
- 수정(`datafile/writer/file.rs`): `FileArrowWriter::try_new` 가 footer 를 직접 쓴다. `without_view_types()` 로 `Utf8View→Utf8`, `BinaryView→Binary`, `Dictionary→값 타입` 으로 바꾼 스키마를 `add_encoded_arrow_schema_to_metadata` 에 넣고, arrow writer 에는 `skip_arrow_metadata=true` 를 준다. 배치는 캐스트하지 않는다(인코딩은 같으니 footer 만 바꾸면 된다). 병렬·직렬 writer 둘 다 같은 자리를 지난다.
- 테스트: `without_view_types_replaces_in_memory_types_everywhere`(단위), `test_delete_rewrites_file_with_plain_types_in_arrow_schema`(delete 후 파일을 arrow-rs 로 열면 원래 스키마와 같음). 기존 `command_optimize` 테스트 2 개(`test_zorder_unpartitioned`, `test_optimize_spark_written_nullable_nested_field`)는 읽어 온 컬럼을 `StringViewArray` 로 downcast 하고 있어 `StringArray` 로 바꿨다. 리뷰어가 물을 지점이므로 PR 본문에 적는다.
- 왜 PR 0 인가: V2 테이블이 아니어도 pyarrow 로 delta-rs 테이블의 파일을 직접 읽는 사람 전부에게 해당한다. 이슈를 먼저 올리고(제목 예: "files rewritten by delete/update/merge/optimize carry view and dictionary types in ARROW:schema") 독립 PR 로 낸다.

### 3-2. `ICEBERG_COMPAT_VERSION` tag → PR 3 에 포함

- Spark 는 V2 테이블에 쓰는 모든 `add` 에 `tags: {"ICEBERG_COMPAT_VERSION": "2"}` 를 단다. `REORG TABLE ... APPLY (UPGRADE UNIFORM)` 은 이 tag 가 없거나 낮은 파일을 전부 다시 쓴다. PROTOCOL.md 에는 없는 Spark 관례다.
- 수정(`datafile/properties.rs`, `datafile/writer/partition.rs`): `DeltaWriterProperties.file_tags`. `apply_table_stats`(write·optimize 두 경로가 모두 지남)가 V2 가 켜진 테이블이면 채우고, `PartitionWriter` 가 `create_add` 결과에 붙인다. 약 20 줄.
- 테스트: `test_iceberg_compat_v2_writes_nested_field_ids` 에 tag assertion 추가. Spark 쪽은 2 절 8 번.
- 리뷰어가 "스펙에 없으니 빼라" 고 하면 빼도 된다. 대신 기존 테이블에 UniForm 을 켜는 사용자는 REORG 가 파일을 한 번 다 다시 쓴다는 것만 알면 된다.

### 3-3. 바꾸지 않은 것

- **CM 테이블의 `OPTIMIZE` 거부**: `operations/optimize.rs` 의 기존 guard. V2 는 CM 이 필수라 V2 테이블은 delta-rs 로 compaction 을 못 한다. V2 와 별개의 기능 요청이라 PR 범위에 넣지 않고 "known limitation" 으로 적는다. (Spark `OPTIMIZE` 는 됨)
- **Spark 의 `nested.ids: {}` 정규화**: delta-rs 가 그대로 받아들이므로 할 일 없음.
- **변환 시점**: Iceberg 메타데이터는 Spark 커밋이 있을 때만 갱신된다. delta-rs 만 쓰는 동안은 Iceberg 쪽이 뒤처진다. UniForm 자체의 성질이라 PR 본문의 "out of scope" 에 한 줄 적는다.

## 4. 검증 (수정 반영 후, upstream main `f67f1210`)

- 파이프라인(2 절 1~8)을 새 테이블로 처음부터 다시 돌려 전부 통과. 재작성 파일 포함 delta-rs 파일 전부가 pyarrow 에서 `string` 으로 읽힘.
- `cargo test -p deltalake-core --features datafusion --lib` 1360 통과(2 ignored. 기존 4 개 + 이번 2 개 새 테스트 포함), `--test it_datafusion` 167 통과, `--test it` 49 통과.
- `cargo fmt`. `cargo clippy --tests` 경고 5 개는 모두 손대지 않은 파일(main 에 이미 있음).
- `prototype.diff`: 17 파일, +808/−52.

## 5. 다시 돌리기

```bash
# 1. 프로토타입 적용 (plan.md 8 절) 후, integration/ 의 두 테스트를 crates/core/tests/it_datafusion/ 에 복사하고 main.rs 에 mod 추가
cd $WORK && uv venv --python 3.12 .venv && uv pip install --python .venv/bin/python delta-spark "pyiceberg[pyarrow,sqlite]"
cp contribution/iceberg-compat-v2-writer/integration/{spark_uniform.py,pyiceberg_read.py,ivysettings.xml} $WORK/
# 2. delta-rs 가 테이블 생성
UNIFORM_OUT=$WORK/table cargo test -p deltalake-core --features datafusion --test it_datafusion export_uniform_table -- --ignored
# 3. Spark 변환 → pyiceberg
cd $WORK && .venv/bin/python spark_uniform.py table convert && .venv/bin/python pyiceberg_read.py table
# 4. 양방향 쓰기
.venv/bin/python spark_uniform.py table insert
UNIFORM_OUT=$WORK/table cargo test -p deltalake-core --features datafusion --test it_datafusion roundtrip_uniform_table -- --ignored
.venv/bin/python spark_uniform.py table touch && .venv/bin/python pyiceberg_read.py table 1,3,4,5,6
.venv/bin/python spark_uniform.py table reorg   # 새 커밋이 생기지 않아야 함
```

`JAVA_TOOL_OPTIONS` 가 설정돼 있으면 비워 두고 돌린다(Spark 로그에 섞임).
