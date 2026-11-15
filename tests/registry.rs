//! `registry.json` lists every widget file, and every widget file starts with its header (purpose, props,
//! states, messages, usage).
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
