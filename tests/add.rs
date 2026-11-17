//! `amenui add`: a component's files and its dependencies' land in the project, at their paths; an
//! unknown name says which one was meant. A material copied out and added back draws as the built-in one.
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

#[test]
fn a_material_copied_out_and_added_back_draws_the_frame_the_built_in_one_draws() {
    use amenui::style::{Look, Material, MaterialParams};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = amenui::add::material_files(amenui::sources::REGISTRY, &["smudgr"]).expect("resolves");
    assert_eq!(files, ["src/materials/smudgr.wgsl"]);
    let to = root.join("target/add-material-test");
    let _ = std::fs::remove_dir_all(&to);
    amenui::add::write(&to, &files, false).expect("copies");
    let src = std::fs::read_to_string(to.join("src/materials/smudgr.wgsl")).unwrap();
    let e = amenui::add::material_files(amenui::sources::REGISTRY, &["smudgy"]).expect_err("unknown");
    assert!(e.to_string().contains("`smudgr`"), "{e}");
    // Added back under a name of its own, with the built-in one's numbers.
    let m = Look::material_source("my_smudgr", &src, MaterialParams::defaults(Material::Smudgr)).expect("adds");
    assert!(matches!(m, Material::Custom(9)), "{m:?}");
    assert_eq!(m.name(), "my_smudgr");
    assert_eq!(amenui::style::material_named("my_smudgr"), Ok(m));
    assert_eq!(Look::material_source("my_smudgr", &src, MaterialParams::defaults(Material::Smudgr)), Ok(m), "adding it again keeps its id");
    assert!(Look::material_source("smudgr", &src, MaterialParams::defaults(Material::Smudgr)).unwrap_err().to_string().contains("built-in"));
    assert!(Look::material_source("Mine", &src, MaterialParams::defaults(Material::Smudgr)).is_err());
    let Ok(mut o) = amenui::offscreen::Offscreen::try_new(480, 360) else { eprintln!("no GPU adapter here: skipped"); return };
    let fonts = || amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts");
    let spec = |material: &str| amenui::drive::RenderSpec { page: "recipe-counter".into(), w: 480, h: 360, scale: 1.0, material: Some(material.into()), theme: Some("light".into()), at_ms: 420.0,
        events: amenui::drive::parse_events(r#"[{"at_ms": 200, "kind": "press", "target": "inc"}, {"at_ms": 230, "kind": "release", "target": "inc"}]"#, "inline").unwrap(), locale: None };
    let built_in = amenui::drive::render(&spec("smudgr"), fonts(), Some(&mut o)).expect("render");
    let added = amenui::drive::render(&spec("my_smudgr"), fonts(), Some(&mut o)).expect("render");
    assert!(built_in.png == added.png, "the added copy draws another frame");
    let plain = amenui::drive::render(&spec("plain"), fonts(), Some(&mut o)).expect("render");
    assert!(plain.png != added.png, "the frame shows the material");
}
