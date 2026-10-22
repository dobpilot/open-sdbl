use open_sdbl::metadata::{inflate_raw_deflate, parse_serialized, parse_config_descriptors, Value};
use std::{fs, error::Error};
fn list(v: &Value)->Option<&[Value]>{if let Value::List(v)=v{Some(v)}else{None}}
fn at<'a>(v: &'a Value,p: &[usize])->Option<&'a Value>{p.iter().try_fold(v,|v,i|list(v)?.get(*i))}
fn tag(v:Option<&Value>)->String{match v{Some(Value::Atom(s))=>s.clone(),_=>"?".into()}}
fn len(v:Option<&Value>)->usize{v.and_then(list).map_or(0,|v|v.len())}
fn run()->Result<(),Box<dyn Error>>{
 let args=std::env::args().collect::<Vec<_>>();
 println!("owner_id\tname\txml_attributes\tclass_tag\tclass_len\theader_tag\theader_len\tsdk_attributes\tfound_attribute_descriptors\ttyped_attribute_descriptors\tattribute_shapes");
 for line in fs::read_to_string(&args[1])?.lines(){
  let r=line.split('\t').collect::<Vec<_>>();
  let raw=fs::read(std::path::Path::new(&args[2]).join(r[0]))?;
  let tree=parse_serialized(&inflate_raw_deflate(&raw)?)?;
  let ds=parse_config_descriptors(r[0],&raw)?;
  let owner=ds.iter().find(|d|d.object_guid.as_str()==r[0]).ok_or("missing owner")?;
  let ids=r[3].split(',').filter(|s|!s.is_empty()).collect::<Vec<_>>();
  let native_order_matches=owner.document_attributes.as_ref().is_some_and(|actual| actual.iter().map(|id|id.as_str()).eq(ids.iter().copied()));
  eprintln!("ORDER\t{}\t{}",r[0],native_order_matches);
  let found=ids.iter().filter(|id|ds.iter().any(|d|d.object_guid.as_str()==**id)).count();
  let typed=ids.iter().filter(|id|ds.iter().any(|d|d.object_guid.as_str()==**id&&d.declared_types.is_some())).count();
  let mut shapes=std::collections::BTreeMap::<String,usize>::new();
  if let Some(rows)=list(&tree).and_then(|root|root.iter().find_map(|v|{let r=list(v)?;if tag(r.first())=="45e46cbc-3e24-4165-8b7b-cc98a6f80211"{Some(r)}else{None}})){
   for row in rows.iter().skip(2){let key=format!("w{}/{}:h{}/{}",tag(at(row,&[0,0])),len(at(row,&[0])),tag(at(row,&[0,1,1,1,0])),len(at(row,&[0,1,1,1])));*shapes.entry(key).or_default()+=1;}
  }
  let shapes=shapes.iter().map(|(k,v)|format!("{k}={v}")).collect::<Vec<_>>().join(";");
  println!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",r[0],r[1],r[2],tag(at(&tree,&[1,0])),len(at(&tree,&[1])),tag(at(&tree,&[1,9,1,0])),len(at(&tree,&[1,9,1])),owner.document_attributes.as_ref().map_or("None".into(),|a|a.len().to_string()),found,typed,shapes);
 }
 Ok(())
}
fn main(){if let Err(e)=run(){eprintln!("ERROR: {e}");std::process::exit(1)}}
