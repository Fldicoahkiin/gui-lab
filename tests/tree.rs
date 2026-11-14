//! The tree export (DESIGN.md §1.17) on the card at two widths, and the view problems.
use lab::el::{El, Paint, col, row, space, text, text_lh};
use lab::ui::Ui;

fn font() -> Vec<lab::text::Face> { lab::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
const PARA: &str = "一段用来测量的文字，中文和 Latin words 混排，宽度变窄时会多出几行。";
fn card(_: &()) -> El<()> {
    col().key("root")
        .child(row().key("card").wrap().pad(12.0, 10.0).gap(8.0)
            .child(space(50.0, 30.0).key("a"))
            .child(space(70.0, 40.0).key("b").on_press(()))
            .child(space(30.0, 20.0).key("c")))
        .child(text_lh(PARA, 16.0, 24.0, Paint::TEXT).key("para"))
}

#[test]
fn the_card_at_two_widths_as_data() {
    let mut ui = Ui::new((), card, |_, _| {}, font(), 400, 600, 1.0);
    let t = ui.export_tree();
    assert_eq!(t.key, "root");
    let c = t.find("card").expect("card");
    assert_eq!(c.rect, [0.0, 0.0, 400.0, 60.0]);
    assert_eq!(c.children.iter().map(|n| n.key.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
    assert_eq!(t.find("b").unwrap().rect, [70.0, 10.0, 70.0, 40.0]);
    let b = t.find("b").unwrap();
    assert_eq!((b.kind, b.role, b.focusable, b.response, b.legible), ("control", Some("button"), true, "nudge", true));
    let p = t.find("para").unwrap();
    assert_eq!((p.kind, p.text.as_deref(), p.response, p.recompose), ("text", Some(PARA), "nudge", true));
    assert_eq!(t.find("a").unwrap().response, "loosen");
    ui.resize(150, 600, 1.0);
    let t = ui.export_tree();
    assert_eq!(t.find("card").unwrap().rect, [0.0, 0.0, 150.0, 98.0]);
    assert_eq!(t.find("b").unwrap().rect, [12.0, 48.0, 70.0, 40.0]);
    assert_eq!(t.find("para").unwrap().rect[1], 98.0);
    // As JSON.
    let v = serde_json::to_value(&t).unwrap();
    assert_eq!(v["children"][0]["key"], "card");
    assert_eq!(v["children"][0]["children"][1]["role"], "button");
    // Focus shows.
    let (x, y) = ui.node("b").unwrap().rect.center();
    ui.press(x, y);
    assert!(ui.export_tree().find("b").unwrap().focused);
    assert!(ui.problems().is_empty(), "{:?}", ui.problems());
}

fn dup(_: &()) -> El<()> { col().key("root").child(text("a", 14.0, Paint::TEXT).key("x")).child(text("b", 14.0, Paint::TEXT).key("x")).child(row().key("menu").anchor("nothing")) }

#[test]
fn problems_name_the_place_and_the_fix() {
    let ui = Ui::new((), dup, |_, _| {}, font(), 400, 300, 1.0);
    let p: Vec<String> = ui.problems().iter().map(|p| p.to_string()).collect();
    assert!(p.iter().any(|m| m.contains("`x` is used by two elements") && m.contains("root › x") && m.contains(".key(format!")), "{p:?}");
    assert!(p.iter().any(|m| m.contains("anchored to `nothing`")), "{p:?}");
}
