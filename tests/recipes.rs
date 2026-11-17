//! The recipes (docs/recipes/) cannot rot: every one compiles (it is part of the library), lays out,
//! answers its own interaction, and the code block in its page is exactly its file in `examples/`.
use amenui::drive::{Input, open, play, Event};
use amenui::recipes::{NAMES, RECIPES};

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
fn root() -> String { env!("CARGO_MANIFEST_DIR").to_owned() }

#[test]
fn every_recipe_page_shows_its_file_exactly() {
    for (name, src, _) in RECIPES {
        let md = std::fs::read_to_string(format!("{}/docs/recipes/{name}.md", root())).unwrap_or_else(|e| panic!("docs/recipes/{name}.md: {e}"));
        let start = md.find("```rust\n").unwrap_or_else(|| panic!("{name}.md has no ```rust block")) + 8;
        let end = start + md[start..].find("```\n").unwrap_or_else(|| panic!("{name}.md: the rust block is not closed"));
        assert!(md[start..end] == *src, "docs/recipes/{name}.md differs from examples/{name}.rs: copy the file into the block");
        assert!(md.contains(&format!("examples/{name}.rs")));
    }
}

#[test]
fn every_recipe_lays_out_and_answers() {
    let ev = |at: f32, input: Input| Event { at_ms: at, input };
    let press = |k: &str| Input::Press { x: None, y: None, target: Some(k.into()), shift: false };
    // Released anywhere: a press already acted (the dialog's save button is gone by the release).
    let release = |_: &str| Input::Release { x: Some(1.0), y: Some(1.0), target: None };
    for name in NAMES {
        let mut d = open(&format!("recipe-{name}"), fonts(), 800, 600, 1.0, None, None).expect("open");
        play(d.as_mut(), &[], 600.0).expect("play");
        let t = d.tree();
        assert!(t.walk().len() > 3, "{name}: an empty tree");
        assert!(d.problems().is_empty(), "{name}: {:?}", d.problems());
        // Its own interaction.
        type Check = Box<dyn Fn(&amenui::tree::TreeNode) -> bool>;
        let (key, check): (&str, Check) = match name {
            "form" => ("form/send", Box::new(|t| t.find("form/sent").and_then(|n| n.text.as_deref()) == Some("Sent: 1"))),
            "list" => ("files/list:3", Box::new(|t| t.find("files/picked").and_then(|n| n.text.as_deref()) == Some("Selected: cover.jpg"))),
            "dialog" => ("rename/save", Box::new(|t| t.find("rename").is_none())),
            "flow" => ("card/1", Box::new(|t| t.find("detail").is_some())),
            "morph" => ("swap", Box::new(|t| t.find("b").is_some())),
            "material" => ("ripple", Box::new(|t| t.find("card").is_some_and(|n| n.material == "halftonr"))),
            "annotate" => ("", Box::new(|t| t.find("panel").is_some())),
            "chapter" => ("cut", Box::new(|t| t.find("scene1").is_some() && t.find("s1/sentence").is_some())),
            "preset" => ("switch", Box::new(|t| t.find("title").and_then(|n| n.text.as_deref()) == Some("A style of its own"))),
            "chart" => ("", Box::new(|t| t.find("chart/d0.0").is_some())),
            "form3d" => ("", Box::new(|t| t.find("passage").is_some())),
            "stack" => ("deck", Box::new(|t| t.find("deck").is_some_and(|n| n.kind == "mesh"))),
            "loading" => ("reload", Box::new(|t| t.find("row4/title").is_some())),
            "submenu" => ("menu/m0/r0", Box::new(|t| t.find("menu/m1").is_some())),
            "gesture" => ("open", Box::new(|t| t.find("sheet/panel").is_some_and(|n| n.role == Some("dialog")))),
            "mesh" => ("turn", Box::new(|t| t.find("turns").and_then(|n| n.text.as_deref()) == Some("Turns: 1") && t.find("ring").is_some_and(|n| n.kind == "mesh"))),
            _ => ("inc", Box::new(|t| t.find("count").and_then(|n| n.text.as_deref()) == Some("10"))),
        };
        if !key.is_empty() { play(d.as_mut(), &[ev(700.0, press(key)), ev(720.0, release(key))], 2000.0).unwrap_or_else(|e| panic!("{name}: {e}")); }
        assert!(check(&d.tree()), "{name}: the interaction did not do what the recipe says");
    }
}

#[test]
fn every_recipe_renders() {
    let Ok(mut o) = amenui::offscreen::Offscreen::try_new(640, 480) else { eprintln!("no GPU adapter here: skipped"); return };
    for name in NAMES {
        let s = amenui::drive::RenderSpec { page: format!("recipe-{name}"), w: 640, h: 480, scale: 1.0, material: None, theme: None, at_ms: 800.0, events: vec![], locale: None };
        let r = amenui::drive::render(&s, fonts(), Some(&mut o)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(r.png.len() > 2000, "{name}: an empty picture");
    }
}
