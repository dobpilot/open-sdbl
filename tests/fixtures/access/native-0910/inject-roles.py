from pathlib import Path
import sys
import os
import xml.etree.ElementTree as ET

root = Path(sys.argv[1])
roles = [
    ("ControlAdmin", "0f618a99-d28c-4bf5-b329-4129462b46f0", True),
    ("ControlRead", "c460a6f2-9e50-4d38-8f5d-47601a023a5b", False),
]
if os.environ.get("CONTROL_EXTRA_ROLE", "false") == "true":
    roles.append(("ControlExtra", "c6c3d518-0852-4301-96f9-782816a893a2", False))
configuration = root / "Configuration.xml"
source = configuration.read_text(encoding="utf-8-sig")
namespace = "{http://v8.1c.ru/8.3/MDClasses}"
configuration_name = ET.fromstring(source).find(f"{namespace}Configuration/{namespace}Properties/{namespace}Name").text
for name, identity, full in roles:
    if f"<Role>{name}</Role>" not in source:
        source = source.replace("<ChildObjects>", f"<ChildObjects>\n<Role>{name}</Role>", 1)
    (root / "Roles").mkdir(exist_ok=True)
    (root / "Roles" / f"{name}.xml").write_text(f'''<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.19">
<Role uuid="{identity}"><Properties><Name>{name}</Name><Synonym/><Comment/></Properties></Role>
</MetaDataObject>''')
    destination = root / "Roles" / name / "Ext"
    destination.mkdir(parents=True, exist_ok=True)
    if full:
        rights = Path("/home/kan/Документы/develop/open-sdbl/tests/fixtures/access/native-095/FullProbe.Rights.xml").read_text(encoding="utf-8-sig")
    else:
        rights = Path("/home/kan/Документы/develop/open-sdbl/tests/fixtures/access/native-095/DefaultRefuse.Rights.xml").read_text(encoding="utf-8-sig")
        start = rights.index("\t<object>", rights.index("<independentRightsOfChildObjects>"))
        end = rights.index("\t</object>", start) + len("\t</object>")
        configuration_rights = rights[start:end]
        configuration_rights = configuration_rights.replace("Configuration.Configuration", f"Configuration.{configuration_name}")
        attribute_defaults = os.environ.get("CONTROL_ATTRIBUTE_DEFAULT", "false")
        owner_read = os.environ.get("CONTROL_OWNER_READ", "true")
        assert attribute_defaults in ("true", "false") and owner_read in ("true", "false")
        restriction = ""
        if os.environ.get("CONTROL_RLS", "false") == "true":
            generated = Path("/home/kan/Документы/develop/open-bsl/docs/research/fixtures/accumulation-field-access/rls-generated-input/Roles/ControlRls/Ext/Rights.xml").read_text(encoding="utf-8-sig")
            restriction = generated[generated.index("<restrictionByCondition>"):generated.index("</restrictionByCondition>") + len("</restrictionByCondition>")]
            category = "a" if name == "ControlExtra" else os.environ.get("CONTROL_RLS_CATEGORY", "b")
            assert category in ("a", "b")
            restriction = restriction.replace("&quot;b&quot;", f"&quot;{category}&quot;")
        rights = f'''<?xml version="1.0" encoding="UTF-8"?>
<Rights xmlns="http://v8.1c.ru/8.2/roles" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:type="Rights" version="2.20">
<setForNewObjects>false</setForNewObjects><setForAttributesByDefault>{attribute_defaults}</setForAttributesByDefault><independentRightsOfChildObjects>false</independentRightsOfChildObjects>
{configuration_rights}
<object><name>DataProcessor.Замеры</name><right><name>Use</name><value>true</value></right><right><name>View</name><value>true</value></right></object>
<object><name>AccumulationRegister.ЗамерОстатков</name><right><name>Read</name><value>{owner_read}</value>{restriction}</right></object>
</Rights>'''
    (destination / "Rights.xml").write_text(rights.replace('version="2.20"', 'version="2.19"'))
configuration.write_text(source)
