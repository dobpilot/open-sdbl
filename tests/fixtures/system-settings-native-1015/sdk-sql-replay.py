from pathlib import Path
import psycopg2,json,sys
root=Path(__file__).resolve().parent
log=Path(sys.argv[1]).read_text()
receipts=[]
for line in log.splitlines():
 if 'NATIVE_SQL\t' not in line: continue
 line=line[line.index('NATIVE_SQL\t'):]
 _,kind,case,key,sql=line.split('\t',4)
 database='open_bsl_numeric_settings_control_1015' if kind=='numeric' else 'open_bsl_user_settings_control_1013'
 c=psycopg2.connect(host='127.0.0.1',port=55439,dbname=database,user='sdbl_control'); c.set_session(readonly=True)
 try:
  with c.cursor() as q:
   q.execute('SHOW transaction_read_only');assert q.fetchone()[0]=='on'
   q.execute(sql);rows=q.fetchall();assert len(rows)<=1
   assert len(rows)==(0 if key=='absent' or kind=='numeric' and key=='storage' else 1),(kind,case,key,len(rows))
   receipt=dict(kind=kind,case=case,key=key,rows=len(rows),sql=sql,read_only=True)
   if rows:
    raw=bytes(rows[0][0]); path=root/('replay-'+case+'-'+key+'.settingsdata');path.write_bytes(raw);receipt['file']=path.name
    if kind!='numeric':
     index={'root':0,'A':2,'B':4}[case]+(1 if key=='storage' else 0)
     assert raw==(root/f'_systemsettings-{index}.settingsdata').read_bytes()
   receipts.append(receipt)
 finally:c.rollback();c.close()
assert len(receipts)==17,len(receipts)
(root/'sdk-sql-replay.json').write_text(json.dumps(receipts,indent=2))
print('READ ONLY native generated-SQL replay:17 queries, matching string payloads and isolated numeric namespaces')
