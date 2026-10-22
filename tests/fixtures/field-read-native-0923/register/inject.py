from pathlib import Path
import json,sys,shutil,xml.etree.ElementTree as ET
base=Path(__file__).resolve().parent;root=Path(sys.argv[1]);configuration=root/'Configuration.xml';text=configuration.read_text(encoding='utf-8-sig');ns='{http://v8.1c.ru/8.3/MDClasses}';name=ET.fromstring(text).find(f'{ns}Configuration/{ns}Properties/{ns}Name').text
(root/'InformationRegisters').mkdir(exist_ok=True);shutil.copy2(base/'InformationRegister.xml',root/'InformationRegisters/ЗамерПолучить.xml')
if '<InformationRegister>ЗамерПолучить</InformationRegister>' not in text:text=text.replace('<ChildObjects>','<ChildObjects>\n<InformationRegister>ЗамерПолучить</InformationRegister>',1)
day=len(sys.argv)>2 and sys.argv[2]=='day'
if day:
 shutil.copy2(base/'PeriodicInformationRegister.xml',root/'InformationRegisters/ЗамерПолучитьПериод.xml')
 if '<InformationRegister>ЗамерПолучитьПериод</InformationRegister>' not in text:text=text.replace('<ChildObjects>','<ChildObjects>\n<InformationRegister>ЗамерПолучитьПериод</InformationRegister>',1)
for profile in json.loads((base/'profiles.json').read_text()):
 role=profile['name'];identity=profile['uuid'];(root/'Roles').mkdir(exist_ok=True)
 if f'<Role>{role}</Role>' not in text:text=text.replace('<ChildObjects>',f'<ChildObjects>\n<Role>{role}</Role>',1)
 (root/'Roles'/f'{role}.xml').write_text(f'<?xml version="1.0" encoding="UTF-8"?><MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.19"><Role uuid="{identity}"><Properties><Name>{role}</Name><Synonym/><Comment/></Properties></Role></MetaDataObject>')
 dest=root/'Roles'/role/'Ext';dest.mkdir(parents=True,exist_ok=True)
 if profile.get('full'):
  rights=Path('/home/kan/Документы/develop/open-sdbl/tests/fixtures/access/native-095/FullProbe.Rights.xml').read_text(encoding='utf-8-sig')
 else:
  read=str(profile['read']).lower();defaults=str(profile['defaults']).lower()
  rights=f'<?xml version="1.0" encoding="UTF-8"?><Rights xmlns="http://v8.1c.ru/8.2/roles" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:type="Rights" version="2.19"><setForNewObjects>false</setForNewObjects><setForAttributesByDefault>{defaults}</setForAttributesByDefault><independentRightsOfChildObjects>false</independentRightsOfChildObjects><object><name>Configuration.{name}</name><right><name>ThinClient</name><value>true</value></right><right><name>ThickClient</name><value>true</value></right><right><name>MainWindowModeNormal</name><value>true</value></right></object><object><name>DataProcessor.Замеры</name><right><name>Use</name><value>true</value></right><right><name>View</name><value>true</value></right></object><object><name>InformationRegister.ЗамерПолучить</name><right><name>Read</name><value>{read}</value></right></object></Rights>'
 if day and not profile.get('full'):rights=rights.replace('</Rights>',f'<object><name>InformationRegister.ЗамерПолучитьПериод</name><right><name>Read</name><value>{read}</value></right></object></Rights>')
 (dest/'Rights.xml').write_text(rights.replace('version="2.20"','version="2.19"'));saved=base/('day-input-roles' if day else 'input-roles')/role;saved.mkdir(parents=True,exist_ok=True);shutil.copy2(root/'Roles'/f'{role}.xml',saved/'Role.xml');shutil.copy2(dest/'Rights.xml',saved/'Rights.xml')
configuration.write_text(text)
