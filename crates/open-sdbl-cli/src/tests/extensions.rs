//! Tests of the extension store reader on the resources of
//! `_ДемоРасширение` of the «1С:Документооборот» demo base.

use std::path::PathBuf;

use open_sdbl::metadata::{inflate_raw_deflate, parse_extension_index};

use super::*;

const ROOT: &str = "344e701a5292613d188f54a0461ba28cdc4e64a0";
const RIGHTS: &str = "335a19b81dcf015fa848dc376306a344b48e2b38";
const ROLE: &str = "262144f7-02b6-4906-89fd-297cc72fe383";

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/extension")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn index() -> ExtensionIndex {
    let root = inflate_raw_deflate(&fixture(&format!("root_{ROOT}.deflate"))).unwrap();
    ExtensionIndex {
        resources: parse_extension_index(&root).unwrap(),
    }
}

#[test]
fn finds_the_resources_of_a_role_by_name() {
    let index = index();
    assert!(!index.is_empty());
    assert_eq!(
        index.key(&format!("{ROLE}.0")).unwrap().as_hex(),
        RIGHTS,
        "the rights resource of the role"
    );
    assert!(index.key(ROLE).is_some(), "the descriptor of the role");
    assert!(index.key("нет").is_none());
    assert!(ExtensionIndex::default().is_empty());
    assert!(ExtensionIndex::default().key(ROLE).is_none());
}
