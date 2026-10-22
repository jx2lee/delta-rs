# 수정 가이드: OSS Unity Catalog 의 `expiration_time: null` 을 디코딩 못함 (#3966)

결과: PR 4884 머지(`3604f18a`, 2026-10-15), 이슈 #3966 close

난이도: ★☆☆☆☆ (필드 타입 하나를 Option 으로, 사용처 한 곳 수정)

## 1. 문제를 한 줄로

응답 JSON 의 `expiration_time` 이 `null` 이면 serde 가 실패합니다. 필드를 `Option` 으로 바꾸면 끝입니다.

## 2. 코드 읽기

| 파일 | 줄 | 무엇 |
|---|---|---|
| `crates/catalog-unity/src/models.rs` | 434-442 | `TemporaryTableCredentials` 구조체 |
| `crates/catalog-unity/src/datafusion.rs` | 112-126 | `expiration_time` 을 쓰는 유일한 곳 (토큰 캐시 만료 계산) |
| `crates/catalog-unity/src/lib.rs` | `get_temp_table_credentials` 검색 | 요청/응답 경로 (수정 불필요, 흐름 파악용) |

Rust 개념 메모:
- `#[serde(with = "chrono::serde::ts_milliseconds")]` 는 "epoch ms 정수 ↔ DateTime" 변환기. `Option` 버전은 `ts_milliseconds_option`.
- `#[serde(default)]` 는 필드가 아예 없을 때 `Default::default()`(Option 이면 `None`) 을 넣습니다. `null` 은 `Option` 이, **없음** 은 `default` 가 처리하니 둘 다 있어야 합니다.

## 3. 수정

### 3-1. models.rs

```rust
-    #[serde(with = "chrono::serde::ts_milliseconds")]
-    pub expiration_time: DateTime<Utc>,
+    /// `None` when the catalog does not vend an expiry (OSS UC returns `null`).
+    #[serde(default, with = "chrono::serde::ts_milliseconds_option")]
+    pub expiration_time: Option<DateTime<Utc>>,
```

### 3-2. datafusion.rs `expire_after_read`

```rust
-        let time_to_expire = value.expiration_time - Utc::now();
-        tracing::info!("Token {_key} expires in {time_to_expire}");
-        time_to_expire.to_std().ok()
+        let Some(expiration_time) = value.expiration_time else {
+            tracing::info!("Token {_key} has no expiry; keeping it until evicted");
+            return None;
+        };
+        let time_to_expire = expiration_time - Utc::now();
+        tracing::info!("Token {_key} expires in {time_to_expire}");
+        time_to_expire.to_std().ok()
```

moka 의 `Expiry` 에서 `None` 을 돌려주면 만료 시간이 없는 것으로 봅니다. `let ... else` 는 패턴이 안 맞으면 else 블록(반드시 return/continue 등으로 끝나야 함)을 실행하는 문법입니다.

`expiration_time` 을 쓰는 곳이 더 있으면 컴파일 에러로 드러납니다. 2026-10-13 기준으로는 위 한 곳뿐입니다.

## 4. 테스트 (models.rs 의 `#[cfg(test)] mod tests` 에 추가)

```rust
#[test]
fn test_temp_credentials_without_expiration_time() {
    // OSS Unity Catalog returns empty credential for file:// tables (#3966)
    for body in [
        r#"{"expiration_time":null, "url":"file:///tmp/t"}"#,
        r#"{"url":"file:///tmp/t"}"#,
    ] {
        let resp: TableTempCredentialsResponse = serde_json::from_str(body).unwrap();
        let TableTempCredentialsResponse::Success(creds) = resp else {
            panic!("expected Success, got {resp:?}");
        };
        assert!(creds.expiration_time.is_none());
    }

    let resp: TableTempCredentialsResponse =
        serde_json::from_str(r#"{"expiration_time":1765000000000, "url":"file:///tmp/t"}"#)
            .unwrap();
    assert!(matches!(
        resp,
        TableTempCredentialsResponse::Success(TemporaryTableCredentials {
            expiration_time: Some(_),
            ..
        })
    ));
}
```

구조체가 아니라 실제 HTTP 응답을 받는 `TableTempCredentialsResponse`(untagged enum `Success | Error`)로 역직렬화합니다. `Error` 변형인 `ErrorResponse` 는 `error_code`, `message` 가 필수입니다. 그래서 수정 전에는 어느 변형에도 맞지 않아 루프 안의 `unwrap()` 이 패닉하고(=재현), 수정 후에는 `Success` 로 디코딩되어 통과합니다.

## 5. 실행

```bash
cargo fmt --all
cargo clippy -p deltalake-catalog-unity --tests
cargo test -p deltalake-catalog-unity
git commit -s -m "fix: use option \`expiration_time\` for oss-uc"
```

## 6. 진행 상황

- 브랜치 `jx2lee/fix/oss-uc-null-expiration-time`, 커밋 `548fb50e`
- PR: https://github.com/delta-io/delta-rs/pull/4884 (2026-10-15 생성, 같은 날 `3604f18a` 로 머지). PR 제목은 `fix(catalog-unity): accept null expiration_time in temporary credentials` 이고 DCO·semantic-release 체크를 통과했습니다.

## 7. 주의

- 같은 crate 의 `MissingCredential` 관련 PR(#4437) 은 "전부 AI 생성" 으로 거절됐습니다. 이 PR 은 이슈 작성자가 원인을 이미 지목했고 수정도 작아서, 본문에 OSS 스펙 링크와 테스트만 넣고 짧게 썼습니다.
- 이 crate 는 `deltalake-core` 와 버전이 따로라 CHANGELOG 는 고치지 않았습니다.
