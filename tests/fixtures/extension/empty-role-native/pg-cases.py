from pathlib import Path
import subprocess, os, zlib,re,base64,json
w=Path('/tmp/sdbl-extension-rights-evidence-1013');rootkey='80408d2f4fedf3485193db5b878103586fa33493';desckey='52ee233472b3581a75f2bcaac5d357828bdb1542';rightkey='1111111111111111111111111111111111111111';role='2bc105dd-b388-436d-9410-a756e3415d30'
def compress(raw):
 c=zlib.compressobj(wbits=-15);return c.compress(raw)+c.flush()
original=(w/'root-1.deflate').read_bytes();raw=zlib.decompress(original,-15)
m=re.search(rb'\{105,[^{}]*\}',raw);assert m
inserted=m[0].replace(b'{105,',b'{106,',1)[:-1]+(',"'+role+'.0",'+base64.b64encode(bytes.fromhex(rightkey)).decode()+'}').encode()
declared=compress(raw[:m.start()]+inserted+raw[m.end():])
descriptor=(w/'role.deflate').read_bytes();body=(w/'role.txt').read_bytes()
promised=(w/'empty-extracted/0'/role).read_bytes()
fixture=Path('/tmp/open-sdbl-extension-rights-recon-1013/tests/fixtures/extension/rights_335a19b81dcf015fa848dc376306a344b48e2b38.deflate').read_bytes()
cases=[('no-subobject','empty',original,descriptor,None),('promised-missing','missing',original,promised,None),('unknown-profile','missing',original,compress(body.replace(b'{6,',b'{7,',1)),None),('descriptor-missing','missing',original,None,None),('descriptor-corrupt','missing',original,b'not deflate',None),('identity-mismatch','missing',original,compress(body.replace(role.encode(),b'11111111-1111-4111-8111-111111111111')),None),('borrowed-profile','missing',original,compress(body.replace(b'{1,0,'+role.encode(),b'{0,0,'+role.encode())),None),('declared-rights-missing','missing',declared,descriptor,None),('declared-rights-corrupt','corrupt',declared,descriptor,b'not deflate'),('declared-rights-stored','stored',declared,descriptor,fixture),('declared-rights-no-descriptor','stored',declared,None,fixture)]
receipts=[]
for name,mode,index,desc,rights in cases:
 sql='TRUNCATE configcas;'
 for key,data in [(rootkey,index),(desckey,desc),(rightkey,rights)]:
  if data is not None:sql+="INSERT INTO configcas VALUES('"+key+"',0,decode('"+data.hex()+"','hex'));"
 r=subprocess.run(['psql','-X','-v','ON_ERROR_STOP=1','-h','127.0.0.1','-p','55439','-U','sdbl_control','-d','sdbl_empty_role_control'],input=sql,text=True,capture_output=True);assert r.returncode==0,r.stderr
 env=os.environ.copy();env['CONTROL_CASE']=mode
 p=subprocess.run(['/tmp/open-sdbl-enum-header19-fix-1013/target/debug/sdbl-empty-role-db-control'],env=env,capture_output=True,text=True,timeout=30)
 receipts.append({'case':name,'expected':mode,'exit':p.returncode,'stdout':p.stdout,'stderr':p.stderr});(w/'db-green-cases.json').write_text(json.dumps(receipts,indent=2));print(name,p.returncode,flush=True);assert p.returncode==0,p.stderr
