use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

use arrow_array::{Int32Array, RecordBatch, StructArray};
use arrow_schema::{DataType as ArrowDataType, Field, Fields, Schema as ArrowSchema};
use deltalake_core::kernel::DataType;
use deltalake_core::operations::optimize::OptimizeType;
use deltalake_core::table::config::TableProperty;
use deltalake_core::DeltaTable;

// #3299
#[tokio::test]
async fn repro_3299_uniform_properties() -> Result<(), Box<dyn Error>> {
    println!("parse enableIcebergCompatV2: {:?}", "delta.enableIcebergCompatV2".parse::<TableProperty>().is_ok());
    println!("parse universalFormat.enabledFormats: {:?}", "delta.universalFormat.enabledFormats".parse::<TableProperty>().is_ok());
    let config: HashMap<String, Option<String>> = [
        ("delta.enableIcebergCompatV2".to_string(), Some("true".to_string())),
        ("delta.universalFormat.enabledFormats".to_string(), Some("iceberg".to_string())),
    ].into_iter().collect();
    let res = DeltaTable::new_in_memory()
        .create()
        .with_column("x", DataType::INTEGER, true, None)
        .with_configuration(config)
        .await;
    println!("create result: {:?}", res.as_ref().err());
    Ok(())
}

// #4363
#[tokio::test]
async fn repro_4363_zorder_dotted_column() -> Result<(), Box<dyn Error>> {
    let schema = Arc::new(ArrowSchema::new(vec![
        Field::new("a.b", ArrowDataType::Int32, true),
        Field::new("n", ArrowDataType::Struct(Fields::from(vec![Field::new("a", ArrowDataType::Int32, true)])), true),
    ]));
    let n = StructArray::from(vec![(
        Arc::new(Field::new("a", ArrowDataType::Int32, true)),
        Arc::new(Int32Array::from(vec![3, 2, 1])) as _,
    )]);
    let batch = RecordBatch::try_new(schema, vec![Arc::new(Int32Array::from(vec![1, 2, 3])), Arc::new(n)])?;
    let table = DeltaTable::new_in_memory().write(vec![batch]).await?;
    for cols in [vec!["a.b"], vec!["`a.b`"], vec!["n.a"]] {
        let mut t = DeltaTable::new(table.log_store());
        t.load().await?;
        let res = t
            .optimize()
            .with_type(OptimizeType::ZOrder(cols.iter().map(|c| c.to_string()).collect()))
            .await;
        match res {
            Ok((_, m)) => println!("zorder {cols:?}: OK files_added={}", m.num_files_added),
            Err(e) => println!("zorder {cols:?}: ERR {e}"),
        }
    }
    Ok(())
}
