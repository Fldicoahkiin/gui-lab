//! `amenui add`: a component's files and its dependencies' land in the project, at their paths; an
//! unknown name says which one was meant.
#[test]
fn add_copies_a_component_and_what_it_needs() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let reg = amenui::sources::REGISTRY;
    let files = amenui::add::files(reg, &["dialog"]).expect("resolves");
    for f in ["src/el.rs", "src/widgets/mod.rs", "src/widgets/button.rs", "src/widgets/dialog.rs"] { assert!(files.iter().any(|x| x == f), "{f} in {files:?}"); }
    let to = root.join("target/add-test");
    let _ = std::fs::remove_dir_all(&to);
    let wrote = amenui::add::write(&to, &files, false).expect("copies");
    assert_eq!(wrote.len(), files.len());
    assert_eq!(std::fs::read_to_string(to.join("src/widgets/dialog.rs")).unwrap(), std::fs::read_to_string(root.join("src/widgets/dialog.rs")).unwrap());
    // Again: nothing is overwritten.
    assert!(amenui::add::write(&to, &files, false).unwrap().is_empty());
    let e = amenui::add::files(reg, &["buton"]).expect_err("unknown");
    assert!(e.to_string().contains("Did you mean `button`"), "{e}");
    assert!(amenui::add::files(reg, &["separator"]).unwrap().iter().any(|f| f.ends_with("divider.rs")), "an alias");
}

#[test]
fn every_file_the_registry_lists_is_compiled_in_and_current() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let reg: serde_json::Value = serde_json::from_str(amenui::sources::REGISTRY).unwrap();
    let names: Vec<&str> = reg["items"].as_array().unwrap().iter().filter_map(|i| i["name"].as_str()).collect();
    for f in amenui::add::files(amenui::sources::REGISTRY, &names).unwrap() {
        let src = amenui::sources::file(&f).unwrap_or_else(|| panic!("{f} is not in amenui::sources::file"));
        assert_eq!(src, std::fs::read_to_string(root.join(&f)).unwrap(), "{f}");
    }
}
