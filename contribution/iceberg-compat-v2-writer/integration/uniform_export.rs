//! Scratch helper: writes an icebergCompatV2 table to $UNIFORM_OUT for the Spark UniForm check.
use std::io::Cursor;
use std::sync::Arc;

use deltalake_core::arrow::datatypes::Schema as ArrowSchema;
use deltalake_core::arrow::json::ReaderBuilder;
use deltalake_core::kernel::engine::arrow_conversion::TryIntoArrow;
use deltalake_core::kernel::{ArrayType, DataType, MapType, StructField, StructType};
use deltalake_core::{DeltaTableBuilder, TableProperty};

#[tokio::test]
#[ignore]
async fn export_uniform_table() {
    let out = std::env::var("UNIFORM_OUT").expect("UNIFORM_OUT");
    let int_array = || DataType::Array(Box::new(ArrayType::new(DataType::INTEGER, true)));
    let columns = vec![
        StructField::new("id", DataType::LONG, true),
        StructField::new("name", DataType::STRING, true),
        StructField::new("flag", DataType::BOOLEAN, true),
        StructField::new("amount", DataType::decimal(10, 2).unwrap(), true),
        StructField::new("ratio", DataType::DOUBLE, true),
        StructField::new("d", DataType::DATE, true),
        StructField::new("ts", DataType::TIMESTAMP, true),
        StructField::new("arr", int_array(), true),
        StructField::new(
            "m",
            DataType::Map(Box::new(MapType::new(DataType::STRING, int_array(), true))),
            true,
        ),
        StructField::new(
            "s",
            DataType::Struct(Box::new(
                StructType::try_new([
                    StructField::new("inner", int_array(), true),
                    StructField::new("note", DataType::STRING, true),
                ])
                .unwrap(),
            )),
            true,
        ),
        StructField::new("p", DataType::STRING, true),
    ];
    let table = DeltaTableBuilder::from_url(deltalake_core::ensure_table_uri(&out).unwrap())
        .unwrap()
        .build()
        .unwrap()
        .create()
        .with_columns(columns)
        .with_partition_columns(["p"])
        .with_configuration_property(TableProperty::ColumnMappingMode, Some("name"))
        .with_configuration_property(TableProperty::EnableIcebergCompatV2, Some("true"))
        .with_configuration_property(
            TableProperty::UniversalFormatEnabledFormats,
            Some("iceberg"),
        )
        .await
        .unwrap();
    let schema: ArrowSchema = table
        .snapshot()
        .unwrap()
        .schema()
        .as_ref()
        .try_into_arrow()
        .unwrap();
    let schema = Arc::new(schema);
    let rows = |json: &str| -> Vec<_> {
        ReaderBuilder::new(schema.clone())
            .build(Cursor::new(json.to_owned()))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    let v1 = r#"{"id":1,"name":"a","flag":true,"amount":"1.50","ratio":0.5,"d":"2026-10-19","ts":"2026-10-19T01:02:03Z","arr":[1,2],"m":{"k":[1]},"s":{"inner":[4],"note":"n"},"p":"x"}
{"id":2,"name":"b","flag":false,"amount":"2.25","ratio":1.5,"d":"2026-10-20","ts":"2026-10-20T01:02:03Z","arr":[3],"m":null,"s":{"inner":[5,6],"note":null},"p":"x"}
{"id":3,"name":null,"flag":null,"amount":null,"ratio":null,"d":null,"ts":null,"arr":null,"m":{},"s":null,"p":"y"}
"#;
    let v2 = r#"{"id":4,"name":"d","flag":true,"amount":"-3.00","ratio":2.5,"d":"2026-10-21","ts":"2026-10-21T01:02:03Z","arr":[],"m":{"a":[7],"b":null},"s":{"inner":[],"note":"z"},"p":"y"}
"#;
    let table = table.write(rows(v1)).await.unwrap();
    let table = table.write(rows(v2)).await.unwrap();
    assert_eq!(table.version(), Some(2));
}
