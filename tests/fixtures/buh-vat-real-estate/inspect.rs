use open_sdbl::metadata::{Value,inflate_raw_deflate,parse_serialized,parse_config_descriptors};
use std::{fs,error::Error};
fn dump(v:&Value)->String{match v{Value::Null=>"null".to_string(),Value::List(v)=>format!("[{}]",v.iter().map(dump).collect::<Vec<_>>().join(",")),Value::Atom(s)=>format!("{:?}",s),Value::String(s)=>format!("{:?}",s)}}
fn main()->Result<(),Box<dyn Error>>{let p=std::path::Path::new("/tmp/open-sdbl-vat-property-recovery-inspection-1016");let raw=fs::read(p.join("owner.deflate"))?;let inflated=inflate_raw_deflate(&raw)?;fs::write(p.join("owner.config"),&inflated)?;let v=parse_serialized(&inflated)?;fs::write(p.join("tree.json"),dump(&v))?;let ds=parse_config_descriptors("c5a2db66-da7e-4191-b273-85267e5f1e17",&raw)?;for d in ds{println!("{d:#?}");}Ok(())}
