//! Scratch helper: delta-rs reads/writes the table after Spark touched it ($UNIFORM_OUT).
use std::io::Cursor;
use std::sync::Arc;

use deltalake_core::arrow::datatypes::Schema as ArrowSchema;
use deltalake_core::arrow::json::ReaderBuilder;
use deltalake_core::datafusion::prelude::SessionContext;
use deltalake_core::kernel::engine::arrow_conversion::TryIntoArrow;
use deltalake_core::{DeltaTableBuilder, open_table};

async fn ids(ctx: &SessionContext, path: &str) -> Vec<i64> {
    let table = open_table(deltalake_core::ensure_table_uri(path).unwrap())
        .await
        .unwrap();
    table.update_datafusion_session(&ctx.state()).unwrap();
    ctx.register_table("t", table.table_provider().await.unwrap())
        .unwrap();
    let batches = ctx
        .sql("SELECT id FROM t ORDER BY id")
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    ctx.deregister_table("t").unwrap();
    batches
        .iter()
        .flat_map(|b| {
            b.column(0)
                .as_any()
                .downcast_ref::<deltalake_core::arrow::array::Int64Array>()
                .unwrap()
                .values()
                .to_vec()
        })
        .collect()
}

#[tokio::test]
#[ignore]
async fn roundtrip_uniform_table() {
    let out = std::env::var("UNIFORM_OUT").expect("UNIFORM_OUT");
    let ctx = SessionContext::new();
    // read what Spark appended (version 4: id 5 in partition z)
    assert_eq!(ids(&ctx, &out).await, [1, 2, 3, 4, 5]);

    let table = DeltaTableBuilder::from_url(deltalake_core::ensure_table_uri(&out).unwrap())
        .unwrap()
        .load()
        .await
        .unwrap();
    let schema: ArrowSchema = table
        .snapshot()
        .unwrap()
        .schema()
        .as_ref()
        .try_into_arrow()
        .unwrap();
    let json = r#"{"id":6,"name":"f","flag":false,"amount":"6.00","ratio":4.5,"d":"2026-10-23","ts":"2026-10-23T01:02:03Z","arr":[12],"m":{"r":[13,14]},"s":{"inner":[15],"note":"v"},"p":"z"}
"#;
    let batches: Vec<_> = ReaderBuilder::new(Arc::new(schema))
        .build(Cursor::new(json))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let table = table.write(batches).await.unwrap();
    assert_eq!(table.version(), Some(5));
    // delete rewrites the partition-x file that delta-rs wrote in version 1
    let (table, _) = table.delete().with_predicate("id = 2").await.unwrap();
    assert_eq!(table.version(), Some(6));
    // OPTIMIZE is refused on column-mapped tables (UnsupportedColumnMapping), see plan-integration.md
    let err = table.optimize().await.unwrap_err();
    assert!(err.to_string().contains("OPTIMIZE"), "{err}");

    assert_eq!(ids(&ctx, &out).await, [1, 3, 4, 5, 6]);
}
