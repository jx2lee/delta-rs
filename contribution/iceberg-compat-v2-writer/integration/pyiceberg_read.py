"""Read the UniForm Iceberg metadata that Spark generated for the delta-rs table with pyiceberg."""
import glob
import os
import sys

import pyarrow.parquet as pq
from pyiceberg.table import StaticTable

path = os.path.abspath(sys.argv[1])
import json
metas = sorted(glob.glob(f"{path}/metadata/*.metadata.json"), key=lambda m: json.load(open(m))["last-updated-ms"])
print("metadata:", [os.path.basename(m) for m in metas])
t = StaticTable.from_metadata(metas[-1])
print("delta-version:", t.properties.get("delta-version"))
print("== schema"); print(t.schema())
print("== spec"); print(t.spec())
print("== files (manifest)")
files = t.inspect.files().to_pylist()
for f in sorted(files, key=lambda f: f["file_path"]):
    rel = os.path.relpath(f["file_path"].removeprefix("file:"), path)
    print(" ", rel, "partition=", f["partition"], "records=", f["record_count"], "format=", f["file_format"])
    md = pq.read_metadata(f["file_path"].removeprefix("file:"))
    print("    parquet columns:", [c.name for c in md.schema])
print("== scan")
rows = sorted(t.scan().to_arrow().to_pylist(), key=lambda r: r["id"])
for r in rows:
    print(" ", r)
print("== scan with partition filter p = 'y'")
print(" ", sorted(r["id"] for r in t.scan(row_filter="p = 'y'").to_arrow().to_pylist()))
print("== scan with data filter id >= 3, projected")
print(" ", t.scan(row_filter="id >= 3", selected_fields=("id", "arr", "s")).to_arrow().to_pylist())

expected = {
    1: dict(name="a", flag=True, ratio=0.5, arr=[1, 2], m=[("k", [1])], s={"inner": [4], "note": "n"}, p="x"),
    2: dict(name="b", flag=False, ratio=1.5, arr=[3], m=None, s={"inner": [5, 6], "note": None}, p="x"),
    3: dict(name=None, flag=None, ratio=None, arr=None, m=[], s=None, p="y"),
    4: dict(name="d", flag=True, ratio=2.5, arr=[], m=[("a", [7]), ("b", None)], s={"inner": [], "note": "z"}, p="y"),
    # written by Spark (step insert)
    5: dict(name="e", flag=False, ratio=3.5, arr=[8, 9], m=[("q", [10])], s={"inner": [11], "note": "w"}, p="z"),
    # written by delta-rs after Spark (roundtrip test)
    6: dict(name="f", flag=False, ratio=4.5, arr=[12], m=[("r", [13, 14])], s={"inner": [15], "note": "v"}, p="z"),
}
scalars = {
    1: ("1.50", "2026-10-19", "2026-10-19 01:02:03+00:00"),
    2: ("2.25", "2026-10-20", "2026-10-20 01:02:03+00:00"),
    3: ("None", "None", "None"),
    4: ("-3.00", "2026-10-21", "2026-10-21 01:02:03+00:00"),
    5: ("4.75", "2026-10-22", "2026-10-22 01:02:03+00:00"),
    6: ("6.00", "2026-10-23", "2026-10-23 01:02:03+00:00"),
}
want = [int(x) for x in sys.argv[2].split(",")] if len(sys.argv) > 2 else [1, 2, 3, 4]
assert [r["id"] for r in rows] == want, [r["id"] for r in rows]
for r in rows:
    for k, v in expected[r["id"]].items():
        assert r[k] == v, (r["id"], k, r[k], v)
    assert (str(r["amount"]), str(r["d"]), str(r["ts"])) == scalars[r["id"]], r
print("OK: pyiceberg read matches, ids =", want)
