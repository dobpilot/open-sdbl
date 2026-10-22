use open_sdbl::metadata::{parse_config_descriptors, Value};
use std::{fs,path::Path};
fn main() {
 let argument=std::env::args().nth(1).expect("нужен каталог фикстуры");let directory=Path::new(&argument);
 let descriptors=parse_config_descriptors("c5a2db66-da7e-4191-b273-85267e5f1e17",&fs::read(directory.join("owner.deflate")).unwrap()).unwrap();
 let descriptor=|id:&str|descriptors.iter().find(|d|d.object_guid.as_str()==id).unwrap();
 let owner=descriptor("c5a2db66-da7e-4191-b273-85267e5f1e17");assert_eq!(owner.name,"ВосстановлениеНДСПоОбъектамНедвижимости");
 let oracle=fs::read_to_string(directory.join("xml-types-flat.tsv")).unwrap();let rows=oracle.lines().map(|s|s.split('\t').collect::<Vec<_>>()).collect::<Vec<_>>();
 let section_oracle=fs::read_to_string(directory.join("xml-sections.tsv")).unwrap();let sections=section_oracle.lines().map(|s|s.split('\t').collect::<Vec<_>>()).collect::<Vec<_>>();
 let mut actual=String::new();let mut field_count=0;
 let mut check_fields=|scope:&str,ids:&Vec<open_sdbl::metadata::Guid>| {
  let expected=rows.iter().filter(|r|r[0]==scope&&r[3]=="0").map(|r|r[1]).collect::<Vec<_>>();assert_eq!(ids.iter().map(|id|id.as_str()).collect::<Vec<_>>(),expected,"{scope}");
  for id in ids {let field=descriptor(id.as_str());field_count+=1;
   for (index,t) in field.declared_types.as_ref().expect("нужен полный состав типов").iter().enumerate() {
    let qualifiers=t.qualifiers.iter().map(|v|match v{Value::Atom(s)|Value::String(s)=>s.as_str(),_=>panic!("неизвестный квалификатор")}).collect::<Vec<_>>().join(",");
    actual.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\n",scope,id.as_str(),field.name,index,t.tag,t.reference.as_ref().map(|id|id.as_str()).unwrap_or(""),qualifiers));
   }
  }
 };
 check_fields("own",owner.document_attributes.as_ref().expect("нужны десять собственных реквизитов"));
 let ids=owner.document_tabular_sections.as_ref().expect("нужны две табличные части");assert_eq!(ids.iter().map(|id|id.as_str()).collect::<Vec<_>>(),sections.iter().map(|s|s[0]).collect::<Vec<_>>());
 for (id,expected) in ids.iter().zip(&sections){let section=descriptor(id.as_str());assert_eq!(section.name,expected[1]);let fields=section.tabular_section_attributes.as_ref().unwrap();assert_eq!(fields.iter().map(|g|g.as_str()).collect::<Vec<_>>().join(","),expected[2]);check_fields(id.as_str(),fields);}
 assert_eq!(actual,oracle);assert_eq!(field_count,34);assert_eq!(rows.len(),48);println!("native_xml_exact=true; own=10; sections=2; fields=24; qualified_type_rows=48");
}
