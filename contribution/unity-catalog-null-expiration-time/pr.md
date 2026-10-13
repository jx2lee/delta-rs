Branch: `fix/unity-optional-expiration-time`

Title:

```
fix(catalog-unity): accept null expiration_time in temporary credentials
```

Body:

```markdown
# Description

The open-source Unity Catalog server returns `expiration_time: null` for temporary table credentials
of local tables (the field is nullable in its API spec). `TemporaryTableCredentials` declared the field
as a required `DateTime<Utc>` with `ts_milliseconds`, so the response failed to decode and opening any
`uc://` table against the OSS server failed with `error decoding response body`.

Make `expiration_time` an `Option<DateTime<Utc>>` (`ts_milliseconds_option`, default `None`) and
treat a missing expiry in the credential cache as "no expiry" so the credentials are kept until the
cache evicts them. Databricks responses, which always include the timestamp, are unchanged.

# Related Issue(s)

- closes #3966

# Documentation

https://github.com/unitycatalog/unitycatalog/blob/main/api/Models/TemporaryCredentials.md
```

Checklist: `cargo fmt --all`, `cargo clippy -p deltalake-catalog-unity --tests`,
`cargo test -p deltalake-catalog-unity`, `git commit -s`.
