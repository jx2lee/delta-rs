# 수정 가이드: UniForm 속성이 "Error parsing property" 로 거부됨 (#3299)

난이도: ★★☆☆☆ (enum 에 variant 추가 + match 분기 추가. 패턴을 그대로 따라 하면 됨)

## 1. 문제를 한 줄로

delta-rs 는 "아는 `delta.*` 키 목록" 을 enum 으로 들고 있고, 모르는 키는 기본 설정에서 에러를 냅니다.
UniForm 용 4개 키가 그 목록에 없습니다.

## 2. 코드 읽기

| 파일 | 줄 | 무엇 |
|---|---|---|
| `crates/core/src/table/config.rs` | 18-100 | `pub enum TableProperty { AppendOnly, ... }` |
| 같은 파일 | 124-153 | `impl AsRef<str>` : enum → 문자열 |
| 같은 파일 | 155-190 | `impl FromStr` : 문자열 → enum (`_ => Err("unknown config key")`) |
| `crates/core/src/kernel/models/actions.rs` | 466-481 | `apply_properties_to_protocol` 에서 `key.parse::<TableProperty>()` 실패 시 에러 |
| 같은 파일 | 581-612 | `delta.enableDeletionVectors` 를 보고 writer/reader feature 를 켜는 **따라할 본보기** |
| 같은 파일 | 363-420 | `move_table_properties_into_features` 의 문자열 match (여기에도 분기 추가) |

Rust 개념 메모:
- `enum` 에 variant 를 추가하면, 그 enum 을 `match` 하는 모든 곳이 **컴파일 에러**로 빠진 분기를 알려줍니다. 컴파일러를 체크리스트로 쓰면 됩니다.
- `HashSet<TableFeature>` 에 `.insert(..)` 하면 중복 없이 추가됩니다. `TableFeature::IcebergCompatV2` 는 kernel 에 이미 있습니다(`actions.rs:826` 에서 사용 중).

## 3. 수정

### 3-1. `config.rs` 에 variant 4개

```rust
pub enum TableProperty {
    ...
+   /// Whether the table is compatible with Iceberg (UniForm) V1
+   EnableIcebergCompatV1,
+   /// Whether the table is compatible with Iceberg (UniForm) V2
+   EnableIcebergCompatV2,
+   /// Whether the table is compatible with Iceberg (UniForm) V3
+   EnableIcebergCompatV3,
+   /// Formats to generate alongside Delta, e.g. "iceberg"
+   UniversalFormatEnabledFormats,
```

`AsRef<str>` 와 `FromStr` 양쪽에 문자열 매핑을 추가합니다.

```rust
Self::EnableIcebergCompatV1 => "delta.enableIcebergCompatV1",
Self::EnableIcebergCompatV2 => "delta.enableIcebergCompatV2",
Self::EnableIcebergCompatV3 => "delta.enableIcebergCompatV3",
Self::UniversalFormatEnabledFormats => "delta.universalFormat.enabledFormats",
```

```rust
"delta.enableIcebergCompatV1" => Ok(Self::EnableIcebergCompatV1),
"delta.enableIcebergCompatV2" => Ok(Self::EnableIcebergCompatV2),
"delta.enableIcebergCompatV3" => Ok(Self::EnableIcebergCompatV3),
"delta.universalFormat.enabledFormats" => Ok(Self::UniversalFormatEnabledFormats),
```

이것만으로 이슈의 에러는 사라집니다. 하지만 프로토콜상 `icebergCompatV2` 는 **writer feature** 라 아래도 필요합니다.

### 3-2. `actions.rs` `apply_properties_to_protocol` 에 feature 켜기

`delta.enableDeletionVectors` 블록(581-612) 을 복사해서 세 개 키를 처리합니다. DV 와 달리 reader feature 는 없습니다.

```rust
for (prop, feature) in [
    (TableProperty::EnableIcebergCompatV1, TableFeature::IcebergCompatV1),
    (TableProperty::EnableIcebergCompatV2, TableFeature::IcebergCompatV2),
    (TableProperty::EnableIcebergCompatV3, TableFeature::IcebergCompatV3),
] {
    if let Some(value) = parsed_properties.get(&prop) {
        match value.to_ascii_lowercase().parse::<bool>() {
            Ok(true) => {
                let mut features = self.writer_features.take().unwrap_or_default();
                features.insert(feature);
                self.min_writer_version = 7;
                self.writer_features = Some(features);
            }
            Ok(false) => {}
            _ => return Err(Error::Generic(format!(
                "{} = '{value}' is invalid, valid values are ['true', 'false']", prop.as_ref()
            ))),
        }
    }
}
```

`TableFeature::IcebergCompatV3` 가 사용 중인 kernel 버전에 있는지 `grep -rn IcebergCompatV3 ~/.cargo/git/checkouts/delta-kernel-rs-*/*/kernel/src/table_features/` 로 확인하고, 없으면 V1/V2 만 넣으세요.

### 3-3. `move_table_properties_into_features` (363-390) 의 match 에도 추가

```rust
"delta.enableIcebergCompatV1" if parse_bool(value) => Some(TableFeature::IcebergCompatV1),
"delta.enableIcebergCompatV2" if parse_bool(value) => Some(TableFeature::IcebergCompatV2),
```

### 3-4. 범위 밖 (PR 에 "하지 않은 것" 으로 적기)

- Iceberg 메타데이터 생성 자체는 delta-rs 가 하지 않습니다. 속성과 feature 만 기록해 Spark/Databricks 가 이어받게 하는 PR 입니다.
- 프로토콜은 icebergCompatV2 에 `delta.columnMapping.mode` 가 `name`/`id` 일 것을 요구합니다. 검증을 넣을지 리뷰어에게 물어보세요.

## 4. 테스트

Rust (`crates/core/src/kernel/models/actions.rs` 의 기존 `apply_properties_to_protocol` 테스트 1489-1546 옆에):

```rust
#[test]
fn test_iceberg_compat_property_enables_writer_feature() {
    let config = HashMap::from([
        ("delta.enableIcebergCompatV2".to_string(), "true".to_string()),
        ("delta.universalFormat.enabledFormats".to_string(), "iceberg".to_string()),
    ]);
    let protocol = Protocol::default().apply_properties_to_protocol(&config, true).unwrap();
    assert_eq!(protocol.min_writer_version, 7);
    assert!(protocol.writer_features.unwrap().contains(&TableFeature::IcebergCompatV2));
}
```

Python (`python/tests/test_create.py` 근처): 이슈의 코드 그대로 `DeltaTable.create(..., configuration=...)` 가 성공하고
`dt.metadata().configuration` 에 두 키가 있는지, `dt.protocol().writer_features` 에 `icebergCompatV2` 가 있는지 확인.

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
