//! `registry.json` lists every widget file, and every widget file starts with its header (purpose, props,
//! states, messages, usage); every material is listed with its file and reads back by its name, and the
//! names materials had before are refused with the names they have now.
fn root() -> String { env!("CARGO_MANIFEST_DIR").to_owned() }

#[test]
fn every_widget_is_in_the_registry_with_a_header() {
    let reg: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(format!("{}/registry.json", root())).unwrap()).expect("registry.json is JSON");
    let items = reg["items"].as_array().expect("items");
    let files: Vec<String> = items.iter().flat_map(|i| i["files"].as_array().into_iter().flatten().filter_map(|f| f.as_str().map(str::to_owned))).collect();
    for e in std::fs::read_dir(format!("{}/src/widgets", root())).unwrap() {
        let p = e.unwrap().path();
        let rel = format!("src/widgets/{}", p.file_name().unwrap().to_string_lossy());
        assert!(files.contains(&rel), "{rel} is not in registry.json");
        if rel.ends_with("mod.rs") { continue; }
        let src = std::fs::read_to_string(&p).unwrap();
        for part in ["Purpose:", "Props:", "States:", "Messages:", "Usage:"] {
            assert!(src.lines().take_while(|l| l.starts_with("//!")).any(|l| l.contains(part)), "{rel}: the header has no `{part}`");
        }
    }
    for i in items {
        let name = i["name"].as_str().unwrap();
        if name == "base" { continue; }
        assert!(i.get("response").is_some() && i.get("recompose").is_some(), "{name}: no response / recompose in registry.json");
        let groups = ["inputs", "display", "feedback", "navigation", "layout", "data"];
        assert!(name == "base" || i.get("group").and_then(|g| g.as_str()).is_some_and(|g| groups.contains(&g)), "{name}: no group (one of {groups:?}) in registry.json");
        if let Some(r) = i.get("recipe").and_then(|r| r.as_str()) { assert!(std::path::Path::new(&format!("{}/{r}", root())).exists(), "{name}: {r} missing"); }
    }
}

#[test]
fn every_material_reads_back_by_its_name_and_old_names_say_the_new_ones() {
    use amenui::style::{MATERIALS, RENAMED, material_named};
    let reg: serde_json::Value = serde_json::from_str(amenui::sources::REGISTRY).unwrap();
    let listed: Vec<&str> = reg["materials"].as_array().expect("materials").iter().filter_map(|m| m["name"].as_str()).collect();
    for (m, name) in MATERIALS {
        assert_eq!(material_named(name), Ok(m), "{name}");
        assert_eq!(m.name(), name);
        assert!(listed.contains(&name), "{name} is not in registry.json's materials");
        assert!(amenui::materials::source(name).is_some(), "src/materials/{name}.wgsl");
    }
    for (old, new) in RENAMED {
        let e = material_named(old).expect_err(old).to_string();
        assert!(e.contains(&format!("the closest is `{new}`")), "{e}");
    }
    assert!(material_named("halftone").unwrap_err().to_string().contains("halftonr"));
    // A recipe from before the names changed is refused, and says which prefix recipes have now.
    let e = amenui::preset::Preset::from_recipe("look:v1:eyJiYXNlIjoibm9ybWNvcmUiLCJzZXQiOnt9fQ").expect_err("v1").to_string();
    assert!(e.contains("v3") && e.contains("print → smudgr"), "{e}");
    let p = &amenui::preset::presets()[0];
    assert!(p.recipe().starts_with("look:v3:"));
    assert_eq!(amenui::preset::Preset::from_recipe(&p.recipe()).map(|q| q.hash), Ok(p.hash.clone()));
}
