# optimize: `~` 가 들어간 파티션에서 "missing files" 에러 수정 가이드

## 0. 먼저 할 일

이 버그는 아직 GitHub 이슈가 없습니다(2026-10-16 검색).
`issue.md` 의 "Issue draft" 로 이슈부터 올리고, 번호를 받은 다음 작업을 시작합니다.

## 1. 경로는 어떤 모양으로 오가는가

파티션 값이 `a~b` 일 때 같은 파일이 세 가지 모양으로 나타납니다.

| 위치 | 모양 | 만드는 곳 |
|---|---|---|
| 디스크의 실제 파일 | `p=a~b/part-....parquet` | 파티션 값 인코딩 (RFC 3986, `~` 는 그대로) |
| `_delta_log` 의 `add.path` | `p=a~b/part-....parquet` | `serde_path::encode_path` (`~` 는 그대로) |
| 메모리의 `Add.path` | `p=a~b/part-....parquet` | 로그를 읽을 때 percent-decode |

`a b` 처럼 인코딩되는 값이면 로그 쪽 모양이 달라집니다.
디스크는 `p=a%20b`, 로그는 `p=a%2520b`, 메모리는 `p=a%20b` 입니다.

## 2. 버그 위치

optimize 는 합칠 파일 목록을 `FileSelection` 으로 스캔에 넘깁니다.
스캔은 "선택된 파일 id" 와 "활성 파일 id" 를 문자열로 비교하고, 짝이 없는 파일을 missing 으로 처리합니다.

- **선택 쪽** `crates/core/src/delta_datafusion/table_provider/next/mod.rs` 의 `add_path_for_selection`
  - 메모리의 `Add.path`(디코딩된 값)에 `object_store::Path::from` 을 씁니다.
  - `Path::from` 은 object_store 자체 규칙으로 인코딩하고, 이 규칙은 `~` 도 인코딩합니다 → `p=a%7Eb`.
- **활성 쪽** `scan/replay.rs` 의 `apply_file_selection`
  - 로그 원문(`path_raw()`)을 테이블 루트 URL 에 `join` 합니다 → `p=a~b`.

`%7E` 와 `~` 가 달라서 모든 파일이 missing 이 됩니다.
`a b` 는 `Path::from` 이 `%` 를 `%25` 로 바꾸면서 우연히 로그 원문(`%2520`)과 같아지기 때문에 통과합니다.

## 3. 수정

선택 쪽에서도 로그에 쓸 때와 같은 인코더로 원문 모양을 다시 만듭니다.

`crates/core/src/kernel/models/actions.rs` 의 `serde_path` 모듈에서 `encode_path` 를 crate 안에 공개합니다.

```rust
-    fn encode_path(path: &str) -> String {
+    pub(crate) fn encode_path(path: &str) -> String {
         utf8_percent_encode(path, INVALID).to_string()
     }
```

`next/mod.rs` 의 `add_path_for_selection`:

```rust
 fn add_path_for_selection(add: Add) -> String {
     let path = add.path;
     if let Ok(mut url) = Url::parse(&path) {
         strip_url_sensitive_parts(&mut url);
         url.to_string()
     } else {
-        String::from(Path::from(path))
+        crate::kernel::models::actions::serde_path::encode_path(&path)
     }
 }
```

수정 후에는 `next/mod.rs:42` 의 `use object_store::path::Path;` 를 쓰는 곳이 없어지므로 지웁니다(clippy 경고로 확인함).
나머지 clippy 경고는 main 에 원래 있던 것입니다.

## 4. 테스트

`crates/core/tests/it_datafusion/command_optimize.rs` 에 회귀 테스트를 추가합니다.
기존 헬퍼 `setup_test`, `write`, `tuples_to_batch` 를 그대로 씁니다.

```rust
#[tokio::test]
async fn test_optimize_partition_values_with_special_characters() -> Result<(), Box<dyn Error>> {
    for value in ["a~b", "a b", "a%b", "a%20b", "a/b", "한글"] {
        let context = setup_test(true).await?;
        let mut dt = context.table;
        let mut writer = RecordBatchWriter::for_table(&dt)?;
        write(&mut writer, &mut dt, tuples_to_batch(vec![(1, 2)], value)?).await?;
        write(&mut writer, &mut dt, tuples_to_batch(vec![(2, 3)], value)?).await?;

        let (dt, metrics) = dt.optimize().await?;
        assert_eq!(metrics.num_files_removed, 2, "partition value {value:?}");
        assert_eq!(metrics.num_files_added, 1, "partition value {value:?}");
        assert_eq!(dt.get_file_uris()?.count(), 1, "partition value {value:?}");
    }
    Ok(())
}
```

upstream main `3604f18a` 에서 직접 돌려 보니 수정 전에는 실패하고, 수정 후에는 통과했습니다.

## 5. 실행

`dat/` 테스트 데이터가 없으면 `it_datafusion` 이 컴파일되지 않습니다. `make setup-dat` 를 먼저 실행합니다.

```bash
cargo fmt --all
cargo clippy -p deltalake-core --features datafusion --tests
cargo test -p deltalake-core --features datafusion --lib file_selection
cargo test -p deltalake-core --features datafusion --test it_datafusion command_optimize
git commit -s -m "fix: match optimize file selection ids for partition values containing tilde"
```

## 6. 주의

- 범위는 optimize 의 선택 id 하나입니다.
- vacuum 이 같은 값의 파일을 못 지우는 문제는 #4646 이고, PR #4658 이 따로 열려 있습니다. 섞지 마세요.
- CDF 는 #4842 / PR #4843 에서 다룹니다.
- 경로 표현을 타입으로 나누자는 큰 리팩터링은 #4815 입니다. 이 PR 은 그 방향과 충돌하지 않는 국소 수정이라고 본문에 적어 두세요.
- 회귀 시점: Python 1.5.0 은 정상이고 1.5.1 부터 실패합니다. 원인 커밋은 특정하지 않았습니다.
