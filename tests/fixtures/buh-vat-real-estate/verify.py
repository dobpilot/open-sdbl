from pathlib import Path
import json,csv,hashlib,xml.etree.ElementTree as E
p=Path('/tmp/open-sdbl-vat-property-recovery-inspection-1016');native=Path('/home/kan/Документы/develop/open-bsl/target/diagnostics/buh-native-config-1015/native-xml');v=json.loads((p/'tree.json').read_text());ns={'m':'http://v8.1c.ru/8.3/MDClasses','v':'http://v8.1c.ru/8.1/data/core','x':'http://v8.1c.ru/8.3/xcf/readable'};xml=E.parse(p/'Document.xml').getroot().find('m:Document',ns);refs={};profiles=[];rows=[];sections=[]
def typ(typ):
 result=[]
 for t in typ:
  if t.tag.split('}')[-1] not in ('Type','TypeSet'):continue
  val=t.text
  if val=='cfg:AnyIBRef':
   result.append(['#','280f5f0e-9c8a-49cc-bf6d-4d296cc17a63']);continue
  if val.startswith('cfg:'):
   logical=val[4:];kind,name=logical.split('.',1);folder={'CatalogRef':'Catalogs','DocumentRef':'Documents','EnumRef':'Enums','ChartOfAccountsRef':'ChartsOfAccounts','Characteristic':'ChartsOfCharacteristicTypes'}[kind];f=native/folder/(name+'.xml');target=next(n for n in E.parse(f).getroot().findall('.//x:GeneratedType',ns) if n.attrib['name']==logical);gid=target.findtext('x:TypeId',namespaces=ns);refs[logical]={'type_id':gid,'file':str(f),'sha256':hashlib.sha256(f.read_bytes()).hexdigest(),'generated_type_xml':E.tostring(target,encoding='unicode')};result.append(['#',gid]);continue
  if val=='xs:string':
   length=typ.findtext('v:StringQualifiers/v:Length',namespaces=ns);q=[] if length=='0' else [length,{'Variable':'1','Fixed':'0'}[typ.findtext('v:StringQualifiers/v:AllowedLength',namespaces=ns)]];result.append(['S']+q)
  elif val=='xs:boolean':result.append(['B'])
  elif val=='xs:dateTime':result.append(['D']+{'Date':['D'],'Time':['T'],'DateTime':[]}[typ.findtext('v:DateQualifiers/v:DateFractions',namespaces=ns)])
  elif val=='xs:decimal':result.append(['N',typ.findtext('v:NumberQualifiers/v:Digits',namespaces=ns),typ.findtext('v:NumberQualifiers/v:FractionDigits',namespaces=ns),{'Any':'0','Nonnegative':'1'}[typ.findtext('v:NumberQualifiers/v:AllowedSign',namespaces=ns)]])
  else:raise AssertionError(val)
 return result
def attributes(nrows,xrows,scope):
 assert len(nrows)==len(xrows)
 for i,(row,x) in enumerate(zip(nrows,xrows)):
  w=row[0];b=w[1];t=b[1];h=t[1];props=x.find('m:Properties',ns);gid=h[1][2];name=h[2];assert (gid,name)==(x.attrib['uuid'],props.findtext('m:Name',namespaces=ns));assert t[2][0]=='Pattern';expected=typ(props.find('m:Type',ns));assert t[2][1:]==expected,(name,t[2][1:],expected)
  profiles.append([scope,str(i),gid,name,str(len(row)),w[0]+'/'+str(len(w)),b[0]+'/'+str(len(b)),t[0]+'/'+str(len(t)),h[0]+'/'+str(len(h)),h[4],','.join(h[5:])])
  for j,ty in enumerate(expected):rows.append([scope,gid,name,str(j),json.dumps(ty,ensure_ascii=False)])
attributes(v[5][2:],xml.findall('m:ChildObjects/m:Attribute',ns),'own')
xs=xml.findall('m:ChildObjects/m:TabularSection',ns);assert len(xs)==int(v[3][1])==len(v[3])-2
for i,(row,x) in enumerate(zip(v[3][2:],xs)):
 w=row[0];b=w[1];h=b[5][1];assert h[1][2]==x.attrib['uuid'];assert h[2]==x.findtext('m:Properties/m:Name',namespaces=ns);sections.append([str(i),h[1][2],h[2],str(len(row)),w[0]+'/'+str(len(w)),b[0]+'/'+str(len(b)),h[0]+'/'+str(len(h)),row[2][0],row[2][1]]);attributes(row[2][2:],x.findall('m:ChildObjects/m:Attribute',ns),h[2])
for file,header,data in [('attributes.tsv',['scope','index','uuid','name','row_len','wrapper','body','typed','identity','comment','flags'],profiles),('types.tsv',['scope','uuid','name','type_index','native_and_xml_type'],rows),('sections.tsv',['index','uuid','name','row_len','wrapper','body','identity','collection','field_count'],sections)]:
 with (p/file).open('w') as f:w=csv.writer(f,delimiter='\t',lineterminator='\n');w.writerow(header);w.writerows(data)
(p/'reference-types.json').write_text(json.dumps(refs,ensure_ascii=False,indent=2)+'\n');summary={'class':'37/50','owner_identity':'1/7','own_attributes':10,'sections':len(xs),'section_fields':24,'qualified_type_rows':len(rows),'reference_xmls':len(refs),'all_own_and_section_membership_order_names_types_and_qualifiers_match_xml':True,'sdk_descriptors':37,'sdk_known_declarations':0,'sdk_document_attributes':None};(p/'verification.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary,indent=2))
