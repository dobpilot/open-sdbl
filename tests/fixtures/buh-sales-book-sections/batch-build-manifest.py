from pathlib import Path
import xml.etree.ElementTree as E,csv,json
root=Path('/home/kan/Документы/develop/open-bsl/target/diagnostics/buh-native-config-1015/native-xml/Documents');out=Path('/tmp/open-sdbl-sales-book-sections-batch-1016');ns={'m':'http://v8.1c.ru/8.3/MDClasses'};old={r['owner_id']:r for r in csv.DictReader(open('/tmp/open-sdbl-sales-book-inspection-1016/all-documents-after.tsv'),delimiter='\t')};lines=[];data=[]
for file in sorted(root.glob('*.xml')):
 d=E.parse(file).getroot().find('m:Document',ns);id=d.attrib['uuid'];name=d.findtext('m:Properties/m:Name',namespaces=ns);sections=[];nfields=0;own=[a.attrib['uuid'] for a in d.findall('m:ChildObjects/m:Attribute',ns)]
 for s in d.findall('m:ChildObjects/m:TabularSection',ns):
  sid=s.attrib['uuid'];sname=s.findtext('m:Properties/m:Name',namespaces=ns);fields=[(a.attrib['uuid'],a.findtext('m:Properties/m:Name',namespaces=ns))for a in s.findall('m:ChildObjects/m:Attribute',ns)];nfields+=len(fields);sections.append((sid,sname,fields))
 assert id in old
 payload=';'.join(sid+'~'+sname+'~'+','.join(fid+':'+fname for fid,fname in fields) for sid,sname,fields in sections)
 lines.append('\t'.join([id,name,old[id]['class_tag'],old[id]['class_len'],str(len(sections)),str(nfields),','.join(own),payload]));data.append({'owner_id':id,'name':name,'sections':sections,'own_attributes':own})
assert len(lines)==455;(out/'manifest.tsv').write_text('\n'.join(lines)+'\n');(out/'manifest.json').write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n');print('documents',len(lines))
