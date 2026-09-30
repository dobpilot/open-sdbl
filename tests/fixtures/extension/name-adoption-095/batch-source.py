from pathlib import Path
import uuid,json,sys,re
root=Path('/tmp/sdbl095-style-base-after');state=Path('/tmp/sdbl095-batch-state.json')
items=[('Language','Languages','Русский'),('DefinedType','DefinedTypes','GLN'),('CommonCommand','CommonCommands','ИсторияИзменений'),('Catalog','Catalogs','Валюты'),('Enum','Enums','ТипыКонтактнойИнформации')]
stage=sys.argv[1]
records=[]
for kind,folder,name in items:
 a=str(uuid.uuid5(uuid.NAMESPACE_DNS,'sdbl095-'+kind+'-A'));b=str(uuid.uuid5(uuid.NAMESPACE_DNS,'sdbl095-'+kind+'-B'))
 records.append(dict(kind=kind,folder=folder,name=name,a=a,b=b))

def object_xml(kind,name,id,role,stage):
 props=''
 if kind=='Language':
  code = 'ru' if role=='A' else 'de'
  if stage in ('different','case'):code='fr' if role=='A' else 'ru'
  props=f'<LanguageCode>{code}</LanguageCode>'
 elif kind=='DefinedType':props='<Type><v8:Type>xs:string</v8:Type><v8:StringQualifiers><v8:Length>13</v8:Length><v8:AllowedLength>Variable</v8:AllowedLength></v8:StringQualifiers></Type>'
 elif kind=='CommonCommand':props='<Group>FormCommandBarImportant</Group>'
 internal=''
 if kind in ('Catalog','Enum','DefinedType'):
  template=(Path('/tmp/sdbl095-buh-native-extension')/dict((x[0],x[1]) for x in items)[kind]/(dict((x[0],x[2]) for x in items)[kind]+'.xml')).read_text(encoding='utf-8-sig')
  internal=re.search(r'<InternalInfo>.*?</InternalInfo>',template,re.S).group()
  internal=re.sub(r'(<xr:(?:TypeId|ValueId)>)([^<]+)',lambda m:m[1]+str(uuid.uuid5(uuid.NAMESPACE_DNS,id+m[2])),internal)
  internal=internal.replace('.'+dict((x[0],x[2]) for x in items)[kind]+'"','.'+name+'"')
 children='<ChildObjects/>'  if kind in ('Catalog','Enum') else ''
 return f'<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20"><{kind} uuid="{id}">{internal}<Properties><Name>{name}</Name><Synonym/><Comment/>{props}</Properties>{children}</{kind}></MetaDataObject>\n'
old=json.loads(state.read_text()) if state.exists() else []
p=root/'Configuration.xml';config=p.read_text(encoding='utf-8-sig')
for entry in old:
 for role in ('A','B'):
  file=root/entry['folder']/(entry[role+'name']+'.xml')
  if file.exists():file.unlink()
  config=config.replace(f'<{entry["kind"]}>{entry[role+"name"]}</{entry["kind"]}>','')
for entry in records:
 kind=entry['kind'];name=entry['name'];(root/entry['folder']).mkdir(exist_ok=True)
 entry['Aname']=name if stage=='exact' else 'Batch'+kind+'A_R'
 entry['Bname']=name if stage=='different' else name.lower() if stage=='case' else 'Batch'+kind+'B'
 for role in ('A','B'):
  filename=entry[role+'name'];(root/entry['folder']/(filename+'.xml')).write_text(object_xml(kind,filename,entry[role.lower()],role,stage))
  config=config.replace('<ChildObjects>',f'<ChildObjects><{kind}>{filename}</{kind}>',1)
p.write_text(config);state.write_text(json.dumps(records,ensure_ascii=False,indent=2))
