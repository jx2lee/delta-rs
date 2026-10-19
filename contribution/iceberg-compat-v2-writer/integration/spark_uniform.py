"""Register the delta-rs table in Spark, let Delta's UniForm hook write Iceberg metadata."""
import os
import sys

import delta
from delta import configure_spark_with_delta_pip
from pyspark.sql import SparkSession

path = os.path.abspath(sys.argv[1])
work = os.path.dirname(path)
step = sys.argv[2] if len(sys.argv) > 2 else "convert"

builder = (
    SparkSession.builder.master("local[2]")
    .appName("uniform-check")
    .config("spark.sql.extensions", "io.delta.sql.DeltaSparkSessionExtension")
    .config("spark.sql.catalog.spark_catalog", "org.apache.spark.sql.delta.catalog.DeltaCatalog")
    .config("spark.sql.warehouse.dir", f"{work}/warehouse")
    .config("spark.driver.extraJavaOptions", f"-Dderby.system.home={work}/derby")
    .config("javax.jdo.option.ConnectionURL", f"jdbc:derby:;databaseName={work}/derby/metastore_db;create=true")
    .config("spark.databricks.delta.uniform.iceberg.sync.convert.enabled", "true")
    .config("spark.sql.session.timeZone", "UTC")
    .config("spark.jars.ivySettings", f"{os.path.dirname(os.path.abspath(__file__))}/ivysettings.xml")
    .enableHiveSupport()
)
spark = configure_spark_with_delta_pip(
    builder, extra_packages=[f"io.delta:delta-iceberg_2.13:{delta.__version__}"]
).getOrCreate()
spark.sparkContext.setLogLevel("WARN")

if step == "convert":
    spark.sql("DROP TABLE IF EXISTS uni")
    spark.sql(f"CREATE TABLE uni USING DELTA LOCATION '{path}'")
    print("== Spark reads the delta-rs table")
    spark.sql("SELECT * FROM uni ORDER BY id").show(truncate=False)
    print("== detail")
    for r in spark.sql("DESCRIBE DETAIL uni").collect():
        print(" ", r.minReaderVersion, r.minWriterVersion, r.tableFeatures, r.properties)
    print("== first Spark commit -> UniForm conversion hook")
    spark.sql("ALTER TABLE uni SET TBLPROPERTIES ('delta.universalFormat.enabledFormats' = 'iceberg')")
elif step == "insert":
    print("== Spark appends a row to the delta-rs table")
    spark.sql(
        "INSERT INTO uni VALUES (5, 'e', false, 4.75, 3.5, DATE'2026-10-22', TIMESTAMP'2026-10-22 01:02:03', "
        "array(8, 9), map('q', array(10)), named_struct('inner', array(11), 'note', 'w'), 'z')"
    )
    spark.sql("SELECT id, p FROM uni ORDER BY id").show()
elif step == "touch":
    print("== no-op Spark commit so the hook converts the delta-rs versions")
    spark.sql("ALTER TABLE uni SET TBLPROPERTIES ('delta.universalFormat.enabledFormats' = 'iceberg')")
elif step == "reorg":
    print("== REORG TABLE ... APPLY (UPGRADE UNIFORM)")
    spark.sql("REORG TABLE uni APPLY (UPGRADE UNIFORM (ICEBERG_COMPAT_VERSION = 2))").show()

print("== history")
spark.sql("DESCRIBE HISTORY uni").select("version", "operation", "operationParameters", "engineInfo").orderBy("version").show(truncate=False)
print("== iceberg metadata dir")
for root, _, files in os.walk(f"{path}/metadata"):
    for f in sorted(files):
        print(" ", os.path.relpath(os.path.join(root, f), path))
spark.stop()
