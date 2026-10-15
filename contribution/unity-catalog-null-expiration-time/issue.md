# [Bug]: Error when reading table from local OSS Unity Catalog

- Issue: https://github.com/delta-io/delta-rs/issues/3966 (opened 2025-12-05, labels bug, binding/rust, unity-catalog)
- Linked PR: https://github.com/delta-io/delta-rs/pull/4884 (opened 2026-10-15, open, no review yet)
- Verified against main `b597cf23`

## Summary

Opening `uc://unity.default.numbers` against the open-source Unity Catalog server fails with
`Generic error: GET request error: error decoding response body`. The reporter traced it to
`TemporaryTableCredentials.expiration_time`, which the OSS server returns as `null`
(the OSS API spec marks `expiration_time` nullable), while delta-rs requires it.

## Root cause

`crates/catalog-unity/src/models.rs:434-442`:

```rust
pub struct TemporaryTableCredentials {
    pub aws_temp_credentials: Option<AwsTempCredentials>,
    ...
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub expiration_time: DateTime<Utc>,      // <- not Option, no default
    pub url: String,
}
```

`ts_milliseconds` rejects `null` and serde rejects a missing field, so any OSS response without a
timestamp fails to decode. The only consumer is `crates/catalog-unity/src/datafusion.rs:122`
(`TokenExpiry::expire_after_read`), which computes `expiration_time - Utc::now()` for the credential
cache TTL.

## Repro (serde unit test `test_temp_credentials_without_expiration_time`, see guide.md)

Deserialising `{"expiration_time":null, "url":"file:///tmp/t"}` and `{"url":"file:///tmp/t"}`
into `TableTempCredentialsResponse` fails (neither the `Success` nor the `Error` variant matches); the
same body with a numeric `expiration_time` decodes as `Success`.
