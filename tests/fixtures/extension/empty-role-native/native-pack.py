from pathlib import Path
import sys, shutil,zlib,json,hashlib
sys.path.insert(0,'/tmp/open-bsl-v8unpack-1011/src')
from v8unpack.container_writer import build,compress_and_build_simple_file
from v8unpack.container_reader import extract
w=Path('/tmp/sdbl-extension-rights-evidence-1013'); source=Path('/tmp/sdbl-enum19-control-1013/old-modes/Version8_2_16/extracted')
shutil.copytree(source,w/'packed-input')
role='2bc105dd-b388-436d-9410-a756e3415d30';cfg='26c30ece-33b5-498a-b409-a91a046ba66b'
for name in [cfg,'versions']:
 before=zlib.decompress((source/'0'/name).read_bytes(),-15)
 if name==cfg:
  old=b'{09736b02-9cac-4e3f-b4f7-d3e9576ab948,0}'
  assert before.count(old)==1
  after=before.replace(old,('{09736b02-9cac-4e3f-b4f7-d3e9576ab948,1,'+role+'}').encode())
 else:
  assert before.startswith(b'\xef\xbb\xbf{1,7,') or before.startswith(b'{1,7,')
  after=before.replace(b'{1,7,',b'{1,8,',1).rstrip()[:-1]+(',"'+role+'",11111111-1111-4111-8111-111111111111}').encode()
 (w/(name+'.original.txt')).write_bytes(before);p=w/(name+'.modified.txt');p.write_bytes(after)
 compress_and_build_simple_file(str(p),str(w/'packed-input/0'/name))
native=(w/'role.deflate').read_bytes();(w/'packed-input/0'/role).write_bytes(native)
build(str(w/'packed-input'),str(w/'preserved-role.cf'),nested=True)
extract(str(w/'preserved-role.cf'),str(w/'re-extracted'),False,False)
assert (w/'re-extracted/0'/role).read_bytes()==native
assert zlib.decompress(native,-15)==(w/'role.txt').read_bytes()
(w/'preservation.json').write_text(json.dumps({'role':role,'compressed_sha256':hashlib.sha256(native).hexdigest(),'inflated_sha256':hashlib.sha256((w/'role.txt').read_bytes()).hexdigest(),'preserved':True,'rights_resource_added':False,'modified':[cfg,'versions'],'packer':'v8unpack 51473807dc8f874e9b06d84f466fff0861f3c4ac'},indent=2))
