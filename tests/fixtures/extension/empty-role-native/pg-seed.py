from pathlib import Path
import subprocess
w=Path('/tmp/sdbl-extension-rights-evidence-1013')
def run(db,sql):
 r=subprocess.run(['psql','-X','-v','ON_ERROR_STOP=1','-h','127.0.0.1','-p','55439','-U','sdbl_control','-d',db],input=sql,text=True,capture_output=True)
 if r.returncode:raise RuntimeError(r.stderr)
 print(r.stdout)
run('postgres','CREATE DATABASE sdbl_empty_role_control;')
root='80408d2f4fedf3485193db5b878103586fa33493';role='52ee233472b3581a75f2bcaac5d357828bdb1542'
sql='CREATE TABLE _extensionsinfo (_idrref bytea,_extensionorder int,_extname text,_extensionzippedinfo bytea); CREATE TABLE configcas (filename varchar(40),partno int,binarydata bytea);'
sql+="INSERT INTO _extensionsinfo VALUES(decode('00','hex'),0,'OwnControl',decode('00000000"+root+"','hex'));"
for key,name in [(root,'root-1.deflate'),(role,'role.deflate')]:sql+="INSERT INTO configcas VALUES('"+key+"',0,decode('"+(w/name).read_bytes().hex()+"','hex'));"
run('sdbl_empty_role_control',sql)
