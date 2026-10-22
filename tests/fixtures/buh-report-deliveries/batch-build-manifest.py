from pathlib import Path
import xml.etree.ElementTree as E
base=Path('/home/kan/Документы/develop/open-bsl/target/diagnostics/buh-native-config-1015/native-xml/Catalogs');ns={'m':'http://v8.1c.ru/8.3/MDClasses'};out=[]
for f in sorted(base.glob('*.xml')):
 c=E.parse(f).getroot().find('m:Catalog',ns);a=c.findall('m:ChildObjects/m:Attribute',ns);out.append('\t'.join([c.attrib['uuid'],c.findtext('m:Properties/m:Name',namespaces=ns),','.join(x.attrib['uuid']+':'+x.findtext('m:Properties/m:Name',namespaces=ns)for x in a)]))
Path('/tmp/open-sdbl-report-deliveries-batch-1016/manifest.tsv').write_text('\n'.join(out)+'\n');print(len(out))
