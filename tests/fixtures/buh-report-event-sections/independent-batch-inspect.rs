use open_sdbl::metadata::{parse_config_descriptors, inflate_raw_deflate, parse_serialized, Value};
use std::{fs, path::Path, error::Error, hash::{Hash, Hasher}};
fn main() -> Result<(), Box<dyn Error>> {
 let args=std::env::args().collect::<Vec<_>>();
 println!("owner\tname\tclass\tclass_len\tnative_sections\tsdk_sections\tnative_fields\tknown_field_types\tsection_membership_match\tfield_membership_names_match\tprojection_hash");
 for row in fs::read_to_string(&args[1])?.lines() {
  let columns=row.split('\t').collect::<Vec<_>>();
  let raw=fs::read(Path::new(&args[2]).join(columns[0]))?;
  let Value::List(root)=parse_serialized(&inflate_raw_deflate(&raw)?)? else {panic!()};
  let Value::List(class)=&root[1] else {panic!()};
  let tag=match &class[0] { Value::Atom(s)=>s.as_str(), _=>"?" };
  let descriptors=parse_config_descriptors(columns[0],&raw)?;
  let owner=descriptors.iter().find(|d|d.object_guid.as_str()==columns[0]).unwrap();
  let parts=columns[3].split(';').filter(|s|!s.is_empty()).map(|s|s.splitn(3,':').collect::<Vec<_>>()).collect::<Vec<_>>();
  let mut count=0;let mut known=0;let mut fields_match=true;
  for part in &parts {
   let fields=part[2].split(',').filter(|s|!s.is_empty()).map(|s|s.split_once('=').unwrap()).collect::<Vec<_>>();count+=fields.len();
   known+=fields.iter().filter(|(id,_)|descriptors.iter().any(|d|d.object_guid.as_str()==*id&&d.declared_types.is_some())).count();
   fields_match &= descriptors.iter().find(|d|d.object_guid.as_str()==part[0]).is_some_and(|p|p.name==part[1]&&p.tabular_section_attributes.as_ref().is_some_and(|ids|ids.iter().map(|g|g.as_str()).eq(fields.iter().map(|f|f.0))))&&fields.iter().all(|(id,name)|descriptors.iter().any(|d|d.object_guid.as_str()==*id&&d.name==*name));
  }
  let member_match=owner.catalog_tabular_sections.as_ref().is_some_and(|ids|ids.iter().map(|g|g.as_str()).eq(parts.iter().map(|p|p[0])));
  let mut hasher=std::collections::hash_map::DefaultHasher::new();format!("{descriptors:?}").hash(&mut hasher);
  println!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:016x}",columns[0],columns[1],tag,class.len(),parts.len(),owner.catalog_tabular_sections.as_ref().map_or("None".into(),|x|x.len().to_string()),count,known,member_match,fields_match,hasher.finish());
 }
 Ok(())
}
