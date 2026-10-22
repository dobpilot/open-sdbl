use open_sdbl::metadata::parse_config_descriptors;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::args().nth(1).ok_or("нужен каталог ресурсов")?;
    let mut missing = 0;
    for (id, expected) in [
        ("a11dfe2b-ef68-4701-9977-08fb0e680ee1", false),
        ("87c6ea4c-6187-4c3f-92ac-b691c1007e3e", true),
    ] {
        let bytes = std::fs::read(std::path::Path::new(&base).join(format!("{id}.deflate")))?;
        let descriptors = parse_config_descriptors(id, &bytes)?;
        let descriptor = descriptors.iter().find(|descriptor| descriptor.object_guid.as_str() == id)
            .ok_or("описание опции отсутствует")?;
        println!("name={} location={:?} privileged={:?} expected={expected}",
            descriptor.name, descriptor.functional_option_location_guid,
            descriptor.functional_option_privileged_get_mode);
        if descriptor.functional_option_privileged_get_mode != Some(expected) {
            missing += 1;
        }
    }
    if missing > 0 { return Err(format!("не приобретено режимов: {missing}").into()); }
    Ok(())
}
