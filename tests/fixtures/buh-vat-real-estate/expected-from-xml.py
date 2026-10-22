from pathlib import Path
import xml.etree.ElementTree as E
import csv, json, hashlib
# Адаптация существующего verify-xml.py для прямых реквизитов и частей этого документа.
p = Path(__file__).resolve().parent
base = Path('/home/kan/Документы/develop/open-bsl/target/diagnostics/buh-native-config-1015/native-xml')
ns = {'m':'http://v8.1c.ru/8.3/MDClasses','v':'http://v8.1c.ru/8.1/data/core','x':'http://v8.1c.ru/8.3/xcf/readable'}
owner = E.parse(p/'Document.xml').getroot().find('m:Document', ns)
folders = {'CatalogRef':'Catalogs','DocumentRef':'Documents','EnumRef':'Enums','ChartOfAccountsRef':'ChartsOfAccounts','Characteristic':'ChartsOfCharacteristicTypes'}
targets = {}
rows = []
sections = []

def fields(scope, nodes):
    for a in nodes:
        props = a.find('m:Properties',ns)
        name = props.findtext('m:Name',namespaces=ns)
        typ = props.find('m:Type',ns)
        for index, item in enumerate(n for n in typ if n.tag.split('}')[-1] in ('Type','TypeSet')):
            text = item.text
            tag = ref = quals = ''
            if text.startswith('cfg:'):
                logical = text.removeprefix('cfg:')
                kind, obj = logical.split('.',1)
                if logical not in targets:
                    file = base/folders[kind]/(obj+'.xml')
                    generated = next(g for g in E.parse(file).getroot().findall('.//x:GeneratedType',ns) if g.attrib['name']==logical)
                    targets[logical] = {'type_id':generated.findtext('x:TypeId',namespaces=ns),'source_xml':str(file),'source_sha256':hashlib.sha256(file.read_bytes()).hexdigest(),'generated_type_xml':E.tostring(generated,encoding='unicode')}
                tag, ref = '#', targets[logical]['type_id']
            elif text == 'xs:boolean': tag = 'B'
            elif text == 'xs:string':
                tag = 'S'
                q = typ.find('v:StringQualifiers',ns)
                length = q.findtext('v:Length',namespaces=ns)
                allowed = q.findtext('v:AllowedLength',namespaces=ns)
                assert allowed in ('Variable','Fixed')
                quals = '' if length=='0' and allowed=='Variable' else ','.join([length,'1' if allowed=='Variable' else '0'])
            elif text == 'xs:decimal':
                tag = 'N'
                q = typ.find('v:NumberQualifiers',ns)
                sign = q.findtext('v:AllowedSign',namespaces=ns)
                assert sign in ('Any','Nonnegative')
                quals = ','.join([q.findtext('v:Digits',namespaces=ns),q.findtext('v:FractionDigits',namespaces=ns),'0' if sign=='Any' else '1'])
            elif text == 'xs:dateTime':
                tag = 'D'
                q = typ.find('v:DateQualifiers',ns)
                quals = {'Date':'D','Time':'T','DateTime':''}[q.findtext('v:DateFractions',namespaces=ns)]
            else: raise AssertionError(text)
            rows.append([scope,a.attrib['uuid'],name,str(index),tag,ref,quals])

own = owner.findall('m:ChildObjects/m:Attribute',ns)
fields('own',own)
for section in owner.findall('m:ChildObjects/m:TabularSection',ns):
    sid = section.attrib['uuid']
    name = section.findtext('m:Properties/m:Name',namespaces=ns)
    children = section.findall('m:ChildObjects/m:Attribute',ns)
    sections.append([sid,name,','.join(a.attrib['uuid'] for a in children)])
    fields(sid,children)
assert len(own)==10 and len(sections)==2 and len(rows)==48
with (p/'xml-types-flat.tsv').open('w') as out:
    for row in rows: out.write('\t'.join(row)+'\n')
with (p/'xml-sections.tsv').open('w') as out:
    for row in sections: out.write('\t'.join(row)+'\n')
(p/'xml-reference-type-mapping.json').write_text(json.dumps(targets,ensure_ascii=False,indent=2)+'\n')
print(json.dumps({'own':len(own),'sections':len(sections),'type_rows':len(rows),'reference_xmls':len(targets)}))
