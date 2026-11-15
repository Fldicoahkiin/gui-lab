//! `lab add`: a component's files and its dependencies' land in the project, at their paths; an
//! unknown name says which one was meant.
#[test]
fn add_copies_a_component_and_what_it_needs() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let reg = std::fs::read_to_string(root.join("registry.json")).unwrap();
    let files = lab::add::files(&reg, &["dialog"]).expect("resolves");
    for f in ["src/el.rs", "src/widgets/mod.rs", "src/widgets/button.rs", "src/widgets/dialog.rs"] { assert!(files.iter().any(|x| x == f), "{f} in {files:?}"); }
    let to = root.join("target/add-test");
    let _ = std::fs::remove_dir_all(&to);
    let wrote = lab::add::copy(root, &to, &files, false).expect("copies");
    assert_eq!(wrote.len(), files.len());
    assert_eq!(std::fs::read_to_string(to.join("src/widgets/dialog.rs")).unwrap(), std::fs::read_to_string(root.join("src/widgets/dialog.rs")).unwrap());
    // Again: nothing is overwritten.
    assert!(lab::add::copy(root, &to, &files, false).unwrap().is_empty());
    let e = lab::add::files(&reg, &["buton"]).expect_err("unknown");
    assert!(e.to_string().contains("Did you mean `button`"), "{e}");
    assert!(lab::add::files(&reg, &["separator"]).unwrap().iter().any(|f| f.ends_with("divider.rs")), "an alias");
}
