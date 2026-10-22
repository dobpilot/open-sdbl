"""Исследовательская экстракция имеющимся v8unpack, без зависимости продукта.

PYTHONPATH=/tmp/open-sdbl0914-v8unpack/src python3 extract.py
"""
from pathlib import Path
import hashlib,json,tempfile,shutil
from v8unpack.container_reader import extract
root=Path(__file__).resolve().parent
rows=json.loads((root/'extraction.json').read_text())
with tempfile.TemporaryDirectory(prefix='sdbl-field-cf-extract-') as temporary:
    for folder in ['register','document']:
        out=Path(temporary)/folder
        extract(str(root/folder/'control.cf'),str(out),False,False)
        for row in rows:
            if row['container']!=folder+'/control.cf':continue
            data=(out/'1'/row['resource']).read_bytes()
            assert hashlib.sha256(data).hexdigest()==row['sha256']
            (root/row['output']).write_bytes(data)
