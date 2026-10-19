# 수정 가이드: UniForm 속성이 "Error parsing property" 로 거부됨 (#3299)

난이도: ★★☆☆☆ (enum 에 variant 추가. 코드는 쉽지만 protocol feature 를 켤지 말지 메인테이너와 먼저 정해야 함, 3-3 참고)

## 1. 문제를 한 줄로

delta-rs 는 "아는 `delta.*` 키 목록" 을 enum 으로 들고 있고, 모르는 키는 기본 설정에서 에러를 냅니다.
UniForm 용 키(`delta.enableIcebergCompatV1/V2/V3`, `delta.universalFormat.enabledFormats`)가 그 목록에 없습니다.

## 2. 코드 읽기

| 파일 | 줄 | 무엇 |
|---|---|---|
| `crates/core/src/table/config.rs` | 18-121 | `pub enum TableProperty { AppendOnly, ... }` |
| 같은 파일 | 124-153 | `impl AsRef<str>` : enum → 문자열 |
| 같은 파일 | 155-190 | `impl FromStr` : 문자열 → enum (`_ => Err("unknown config key")`) |
| `crates/core/src/kernel/models/actions.rs` | 467-481 | `apply_properties_to_protocol` 에서 `key.parse::<TableProperty>()` 실패 시 에러 |
| 같은 파일 | 581-612 | `delta.enableDeletionVectors` 를 보고 writer/reader feature 를 켜는 코드. 참고만 하고 **따라 하지 않음**(3-2) |
| 같은 파일 | 363-427 | `move_table_properties_into_features` 의 문자열 match (A 안이면 수정 안 함) |
| `crates/core/src/kernel/transaction/protocol.rs` | 231, 322-345 | `can_write_to` 와 delta-rs 가 지원하는 writer feature 목록 |

Rust 개념 메모:
- `enum` 에 variant 를 추가하면, 그 enum 을 `match` 하는 모든 곳이 **컴파일 에러**로 빠진 분기를 알려줍니다. 컴파일러를 체크리스트로 쓰면 됩니다.

## 3. 수정

### 3-1. `config.rs` 에 variant 3개 (V3 는 3-4 참고)

```rust
pub enum TableProperty {
    ...
+   /// Whether the table is compatible with Iceberg (UniForm) V1
+   EnableIcebergCompatV1,
+   /// Whether the table is compatible with Iceberg (UniForm) V2
+   EnableIcebergCompatV2,
+   /// Formats to generate alongside Delta, e.g. "iceberg"
+   UniversalFormatEnabledFormats,
```

`AsRef<str>` 와 `FromStr` 양쪽에 문자열 매핑을 추가합니다.

```rust
Self::EnableIcebergCompatV1 => "delta.enableIcebergCompatV1",
Self::EnableIcebergCompatV2 => "delta.enableIcebergCompatV2",
Self::UniversalFormatEnabledFormats => "delta.universalFormat.enabledFormats",
```

```rust
"delta.enableIcebergCompatV1" => Ok(Self::EnableIcebergCompatV1),
"delta.enableIcebergCompatV2" => Ok(Self::EnableIcebergCompatV2),
"delta.universalFormat.enabledFormats" => Ok(Self::UniversalFormatEnabledFormats),
```

이것만으로 이슈의 에러는 사라집니다. 그런데 여기서 protocol 의 writer feature 까지 켜면 문제가 생깁니다. 아래 3-2 를 먼저 읽고 방향을 정하세요.

### 3-2. writer feature 를 켜면 안 되는 이유 (2026-10-19 PROTOCOL.md 대조)

스펙에서 확인한 사실(`delta-io/delta` master 의 `PROTOCOL.md`):

- "Iceberg Compatibility V1/V2" 절: 이 기능을 지원하려면 Writer Version 7 이어야 하고 `writerFeatures` 에 `icebergCompatV1`/`icebergCompatV2` 가 있어야 합니다. 여기에 `delta.enableIcebergCompatVn = true` 까지 있어야 기능이 켜집니다.
- "Valid Feature Names" 표: 둘 다 `Writers only` 입니다. 읽는 쪽 동작은 바뀌지 않습니다.
- "Supported Features" / "Active Features" 절: `writerFeatures` 에 이름이 있으면 supported, 속성까지 `true` 면 active 입니다. writer 는 `writerFeatures` 에 있는 기능을 **모두 구현해야만** 테이블에 쓸 수 있습니다.
- V2 의 writer 요구사항: column mapping(`name`/`id`) 필수, deletion vector 비활성, array/map 중첩 필드에 field id 부여(`delta.columnMapping.nested.ids`), 파티션 값을 parquet 에 materialize, `numRecords` 통계 필수, timestamp 는 int64, 허용 타입 제한.

delta-rs 쪽 사실:

- `crates/core/src/kernel/transaction/protocol.rs` 322-345 의 writer 지원 목록에 `IcebergCompatV1`/`IcebergCompatV2` 가 **없습니다**.
- 같은 파일 `can_write_to` (231 근처)는 writer version 7 테이블의 `writerFeatures` 중 지원 목록에 없는 것이 있으면 `UnsupportedTableFeatures` 를 냅니다.
- kernel(`buoyant_kernel-0.28.1/src/table_features/mod.rs` 421-457)도 V1/V2 를 `KernelSupport::NotSupported` 로 표시합니다.

`apply_properties_to_protocol` 에서 feature 를 켜면 delta-rs 는 **자기가 만든 테이블에 다음 write 부터 쓰지 못합니다**. 그렇다고 지원 목록에 억지로 넣으면 위 요구사항(중첩 field id, 파티션 materialize 등)을 지키지 않고 쓰게 되니 스펙을 어긴 테이블이 생깁니다.

### 3-3. PR 범위 정하기 (먼저 이슈에 질문)

| 안 | 하는 일 | 결과 | 장단점 |
|---|---|---|---|
| A (추천) | 3-1 만. 키를 파싱만 하고 protocol 은 건드리지 않음 | 속성은 저장되지만 스펙상 기능은 비활성(supported 아님) | 작고 안전. 대신 "켰는데 Iceberg 호환이 아님" 이라는 오해가 생길 수 있음 → 문서·로그로 알려야 함 |
| B | 3-1 + 키가 `true` 면 "delta-rs writer 는 아직 icebergCompatVn 을 지원하지 않음" 명확한 에러 | 지금의 애매한 파싱 에러가 이유 있는 에러로 바뀜 | 이슈 작성자가 원한 "생성 성공" 은 아님 |
| C | writer 요구사항 구현 후 지원 목록에 추가 | 실제로 지원 | ★★★. 이 이슈 범위를 넘지만 생각보다 작음. 계획·검증한 프로토타입은 `../iceberg-compat-v2-writer/` 참고 |

이슈에 남길 질문 예시(영어는 다듬어서 쓰세요):

> `icebergCompatV2` is not in delta-rs' supported writer features, so enabling the feature in the protocol would make the table unwritable by delta-rs itself. Would you accept a PR that only lets these keys parse (A), or prefer an explicit "not supported" error (B)?

A 안에서는 `move_table_properties_into_features`(363 근처)를 **고치지 않습니다**. 여기에 분기를 넣어도 같은 문제가 생깁니다.

### 3-4. V3 는 넣지 않기

- `icebergCompatV3` 는 PROTOCOL.md 본문에 **없습니다**. `protocol_rfcs/iceberg-compat-v3.md` 에 Proposed RFC(2025-05-19, delta-io/delta 이슈 4574)로만 있고, `protocol_rfcs/README.md` 의 Accepted 목록에 없습니다.
- kernel 에는 `TableFeature::IcebergCompatV3` 가 있지만(row tracking 필수, V1/V2 와 달리 DV 허용) 스펙이 확정되지 않았습니다.
- 그래서 `EnableIcebergCompatV3` variant 와 문자열 매핑은 만들지 않고, V1/V2 와 `universalFormat.enabledFormats` 세 키만 다룹니다. 리뷰어가 요청하면 그때 추가하세요.

### 3-5. 범위 밖 (PR 에 "하지 않은 것" 으로 적기)

- Iceberg 메타데이터 생성은 delta-rs 가 하지 않습니다.
- V2 writer 요구사항 구현(3-3 의 C 안)은 별도 이슈로 남깁니다.

## 4. 테스트

Rust (`crates/core/src/kernel/models/actions.rs` 의 기존 `apply_properties_to_protocol` 테스트 1480-1621 옆에):

```rust
#[test]
fn test_iceberg_compat_property_is_accepted_without_feature() {
    let config = HashMap::from([
        ("delta.enableIcebergCompatV2".to_string(), "true".to_string()),
        ("delta.universalFormat.enabledFormats".to_string(), "iceberg".to_string()),
    ]);
    // A 안: 파싱 에러가 없고, protocol 은 기본값 그대로
    let protocol = Protocol::default().apply_properties_to_protocol(&config, true).unwrap();
    assert_eq!(protocol, Protocol::default());
}
```

Python (`python/tests/test_create.py` 근처): 이슈의 코드 그대로 `DeltaTable.create(..., configuration=...)` 가 성공하고
`dt.metadata().configuration` 에 두 키가 있는지 확인합니다. 그다음 `write_deltalake(dt, data, mode="append")` 가 성공하는지도 확인합니다. feature 를 켜는 구현이면 여기서 실패합니다.

## 5. 실행

```bash
cargo fmt --all
cargo clippy -p deltalake-core --features datafusion --tests
cargo test -p deltalake-core --features datafusion apply_properties
git commit -s -m "feat: accept iceberg compat and universal format table properties"
```

## 6. 주의

- 이슈 라벨이 enhancement 이므로 commit prefix 는 `feat:`.
- CONTRIBUTING.md 의 AI 정책: 어떤 부분을 직접 검증했는지(위 Rust 재현 결과) PR 본문에 적으세요.
