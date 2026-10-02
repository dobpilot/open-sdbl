from pathlib import Path
import re,zlib,xml.etree.ElementTree as E,json,sys
sdk=Path(__file__).resolve().parents[3]; xml=Path(sys.argv[1]);ns={'m':'http://v8.1c.ru/8.3/MDClasses','x':'http://v8.1c.ru/8.3/xcf/readable'}
def parse(b):
 t=re.findall(r'"(?:[^"]|"")*"|[{},]|[^\s{},]+',b.decode('utf-8-sig'));i=0
 def item():
  nonlocal i
  x=t[i];i+=1
  if x=='{':
   out=[]
   while t[i]!='}':
    if t[i]==',':i+=1;continue
    out.append(item())
   i+=1;return out
  return x.strip('"').replace('""','"')
 return item()
pack=(sdk/'tests/fixtures/unf/config.pack').read_bytes();off=0;resources={}
while off<len(pack):
 e=pack.index(b'\n',off);g,n=pack[off:e].decode().split('\t');n=int(n);off=e+1;resources[g]=pack[off:off+n];off+=n
regs={}
for kind in ['InformationRegisters','AccumulationRegisters','AccountingRegisters','CalculationRegisters']:
 for p in (xml/kind).glob('*.xml'):
  o=list(E.parse(p).getroot())[0];regs[kind[:-1]+'.'+o.find('m:Properties/m:Name',ns).text]=o.attrib['uuid']
lines=[];docs=parts=attrs=assoc=0
for p in (xml/'Documents').glob('*.xml'):
 o=list(E.parse(p).getroot())[0];g=o.attrib['uuid'];name=o.find('m:Properties/m:Name',ns).text
 if g not in resources:continue
 v=parse(zlib.decompress(resources[g],-15));c=v[1]
 refs=o.findall('m:Properties/m:RegisterRecords/x:Item',ns)
 native=[regs[e.text] for e in refs];encoded=[e[2][1] for e in c[24][2:]]
 assert native==encoded,(name,native,encoded);assert len(c)==53
 lines.append('\t'.join(['D',g,name,','.join(native),','.join(e.text.split('.')[0] for e in refs)]));docs+=1;assoc+=len(native)
 ps=o.findall('m:ChildObjects/m:TabularSection',ns);coll=next(e for e in v if isinstance(e,list) and e[0]=='21c53e09-8950-4b5e-a6a0-1054f1bbc274')
 assert len(ps)==int(coll[1])
 for s,row in zip(ps,coll[2:]):
  sid=s.attrib['uuid'];sname=s.find('m:Properties/m:Name',ns).text; assert row[0][1][5][1][1][2]==sid
  a=[a.attrib['uuid'] for a in s.findall('m:ChildObjects/m:Attribute',ns)]
  ac=next(e for e in row if isinstance(e,list) and e[0]=='888744e1-b616-11d4-9436-004095e12fc7')
  aid=[a[0][1][1][1][1][2] for a in ac[2:]];assert a==aid,(name,sname,a,aid);assert int(ac[1])==len(a)
  lines.append('\t'.join(['T',sid,name+'.'+sname,','.join(a)]));parts+=1;attrs+=len(a)
print('native matched docs/parts/attrs/associations',docs,parts,attrs,assoc,file=sys.stderr)
print('\n'.join(lines))
