"""Извлекает эталон настроек из штатной XML-выгрузки, без SDK-декодера."""
from pathlib import Path
import sys
import xml.etree.ElementTree as E

sdk = Path(__file__).resolve().parents[3]
xml = Path(sys.argv[1])
ns = {"m": "http://v8.1c.ru/8.3/MDClasses"}
if len(sys.argv) > 2:
    selected_names = {p.stem for p in Path(sys.argv[2]).glob("*.deflate")}
    selected_ids = None
else:
    selected_names = None
    selected_ids = set()
    pack = (sdk / "tests/fixtures/unf/config.pack").read_bytes()
    offset = 0
    while offset < len(pack):
        end = pack.index(b"\n", offset)
        identity, size = pack[offset:end].decode().split("\t")
        selected_ids.add(identity)
        offset = end + 1 + int(size)
    assert offset == len(pack)

numerators = {}
for path in (xml / "DocumentNumerators").glob("*.xml"):
    obj = list(E.parse(path).getroot())[0]
    name = obj.find("m:Properties/m:Name", ns).text
    key = "DocumentNumerator." + name
    assert key not in numerators
    numerators[key] = obj.attrib["uuid"]

count = assigned = 0
for path in (xml / "Documents").glob("*.xml"):
    obj = list(E.parse(path).getroot())[0]
    identity = obj.attrib["uuid"]
    props = obj.find("m:Properties", ns)
    name = props.find("m:Name", ns).text
    if selected_ids is not None and identity not in selected_ids:
        continue
    if selected_names is not None and name not in selected_names:
        continue
    periodicity = props.find("m:NumberPeriodicity", ns).text
    numerator_name = props.find("m:Numerator", ns).text
    numerator = numerators[numerator_name] if numerator_name else ""
    print("\t".join([identity, name, periodicity, numerator]))
    count += 1
    assigned += bool(numerator)
print(f"documents={count} assigned_numerators={assigned}", file=sys.stderr)
