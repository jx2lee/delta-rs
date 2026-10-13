# 수정 가이드: 점(.)이 들어간 컬럼 이름을 z-order 할 수 없음 (#4363)

난이도: ★★☆☆☆ (문자열 split 두 곳을 kernel 파서 호출로 바꾸는 작업)

## 1. 문제를 한 줄로

`"a.b"` 를 받으면 무조건 `["a","b"]` 로 쪼개서 중첩 경로로 해석합니다. 이름 자체가 `a.b` 인 컬럼은 영원히 못 찾습니다.
kernel 에 백틱 이스케이프를 이해하는 파서가 이미 있으니 그걸 쓰면 됩니다.

## 2. 코드 읽기

| 파일 | 줄 | 무엇 |
|---|---|---|
| `crates/core/src/operations/optimize.rs` | 762-800 | `read_zorder`: `col_name.split('.')` → `ident(first).field(seg)...` DataFusion 식 생성 |
| 같은 파일 | 1285-1308 | `validate_zorder_column`: `split('.')` 로 스키마 검증 |
| 같은 파일 | 1310-1333 | `build_zorder_plan`: 파티션 컬럼 검사 후 validate 호출 |
| kernel `src/expressions/column_names.rs` | 38-60, 93 | `ColumnName::parse_column_name_list`, `.path() -> &[String]` |
| `crates/core/src/delta_datafusion/data_validation.rs` | 42, 352 | delta-rs 안에서 `ColumnName` 을 이미 쓰는 예 |

Rust 개념 메모:
- `ColumnName::parse_column_name_list("a.`b.c`")` 는 `Vec<ColumnName>` 을 돌려줍니다. 쉼표가 없으면 길이 1. `path()` 로 `&[String]` 세그먼트를 얻습니다.
- `?` 는 `Result` 가 Err 이면 즉시 함수에서 반환. kernel 의 에러 타입은 `DeltaTableError::from` 으로 변환됩니다(`.map_err(DeltaTableError::from)?` 또는 그냥 `?` 가 되는지 컴파일러가 알려줌).

## 3. 수정

### 3-1. 파싱 헬퍼 하나 추가 (optimize.rs 아무 곳, validate 근처)

```rust
use delta_kernel::expressions::ColumnName;

/// Parse one z-order column spec. `a.b` is a nested path, `` `a.b` `` is a single field named "a.b".
fn parse_zorder_column(column: &str) -> Result<Vec<String>, DeltaTableError> {
    let mut names = ColumnName::parse_column_name_list(column).map_err(DeltaTableError::from)?;
    match (names.pop(), names.is_empty()) {
        (Some(name), true) => Ok(name.path().to_vec()),
        _ => Err(DeltaTableError::Generic(format!(
            "Z-order column \"{column}\" must be a single column name"
        ))),
    }
}
```

### 3-2. `validate_zorder_column` 에서 split 대신 사용

```rust
-    let mut segments = column.split('.').peekable();
+    let path = parse_zorder_column(column)?;
+    let mut segments = path.iter().map(String::as_str).peekable();
```

나머지 루프는 그대로 동작합니다.

### 3-3. `read_zorder` 에서도 동일

```rust
-            .map(|col_name| {
-                let mut segments = col_name.split('.');
-                let first = segments.next().expect("column name cannot be empty");
+            .map(|col_name| -> Result<Expr, DeltaTableError> {
+                let path = parse_zorder_column(col_name)?;
+                let mut segments = path.iter();
+                let first = segments.next().expect("validated non-empty above");
                 let mut expr = ident(first);
                 for segment in segments {
                     expr = expr.field(segment);
                 }
-                expr
+                Ok(expr)
             })
-            .collect_vec();
+            .collect::<Result<Vec<_>, _>>()?;
```

`collect::<Result<Vec<_>, _>>()` 는 "하나라도 Err 면 전체 Err" 로 모아 주는 관용구입니다.
`validate_zorder_column` 이 먼저 돌기 때문에 여기서 실제로 실패할 일은 거의 없지만 `expect` 로 패닉을 만들지 않는 편이 리뷰에 유리합니다.

### 3-4. 파티션 컬럼 검사 (1317-1325)

`partition_keys.contains(col)` 는 raw 문자열 비교입니다. `parse_zorder_column(col)?` 의 첫 세그먼트가 파티션 키인지로 바꾸면 ``` `a.b` ``` 같은 입력도 올바르게 걸러집니다. 작은 변경이라 같은 PR 에 넣어도 됩니다.

## 4. 테스트

`crates/core/tests/it_datafusion/command_optimize.rs` 에 아래 재현 코드(2026-10-13 실행해 실패 확인)를 넣고, 수정 후에는
``` `a.b` ``` 가 성공하고 `a.b` 는 여전히 "field a not found" 인지 확인합니다.

```rust
let schema = Arc::new(ArrowSchema::new(vec![
    Field::new("a.b", ArrowDataType::Int32, true),
    Field::new("n", ArrowDataType::Struct(Fields::from(vec![Field::new("a", ArrowDataType::Int32, true)])), true),
]));
let n = StructArray::from(vec![(
    Arc::new(Field::new("a", ArrowDataType::Int32, true)),
    Arc::new(Int32Array::from(vec![3, 2, 1])) as ArrayRef,
)]);
let batch = RecordBatch::try_new(schema, vec![Arc::new(Int32Array::from(vec![1, 2, 3])), Arc::new(n)])?;
let table = DeltaTable::new_in_memory().write(vec![batch]).await?;

let mut t = DeltaTable::new(table.log_store());
t.load().await?;
let (_, m) = t.optimize().with_type(OptimizeType::ZOrder(vec!["`a.b`".into()])).await?;
assert_eq!(m.num_files_added, 1);
```

`validate_zorder_column` 단위 테스트도 1385 줄 근처 `mod tests` 에 두 줄짜리로 추가하면 좋습니다.

## 5. 실행

```bash
cargo fmt --all
cargo clippy -p deltalake-core --features datafusion --tests
cargo test -p deltalake-core --features datafusion zorder
git commit -s -m "feat: parse z-order columns with kernel ColumnName semantics"
```

## 6. 주의

- 이슈에 `take` 가 이미 한 번 있었습니다. 댓글로 "진행 중인 작업이 없으면 제가 하겠다" 고 먼저 남기세요.
- Python 바인딩(`python/deltalake/table.py` 의 `z_order(columns: Iterable[str])`) 은 문자열을 그대로 넘기므로 변경 없이 백틱 문법이 따라옵니다. docstring 에 한 줄 추가하면 친절합니다.
