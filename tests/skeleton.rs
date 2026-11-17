//! Acceptance for the skeleton: layout, pressing, scrolling and focus, all without a GPU.
use amenui::el::{El, Paint, col, row, space, text_lh};
use amenui::ui::{Rect, Ui};

fn font() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

const PARA: &str = "一段用来测量的文字，中文和 Latin words 混排，宽度变窄时会多出几行。";

fn card(_: &()) -> El<()> {
    col().key("root")
        .child(row().key("card").wrap().pad(12.0, 10.0).gap(8.0)
            .child(space(50.0, 30.0).key("a"))
            .child(space(70.0, 40.0).key("b"))
            .child(space(30.0, 20.0).key("c")))
        .child(text_lh(PARA, 16.0, 24.0, Paint::TEXT).key("para"))
}

fn rect(ui: &Ui<(), ()>, k: &str) -> Rect { ui.node(k).unwrap_or_else(|| panic!("no node {k}")).rect }

#[test]
fn card_at_two_widths() {
    let mut ui = Ui::new((), card, |_, _| {}, font(), 400, 600, 1.0);
    assert_eq!(rect(&ui, "card"), Rect::new(0.0, 0.0, 400.0, 60.0));
    assert_eq!(rect(&ui, "a"), Rect::new(12.0, 10.0, 50.0, 30.0));
    assert_eq!(rect(&ui, "b"), Rect::new(70.0, 10.0, 70.0, 40.0));
    assert_eq!(rect(&ui, "c"), Rect::new(148.0, 10.0, 30.0, 20.0));
    let wide = rect(&ui, "para");
    assert_eq!((wide.x, wide.y, wide.w), (0.0, 60.0, 400.0));

    // 126 px inside the padding: a alone on the first line, b and c on the second.
    ui.resize(150, 600, 1.0);
    assert_eq!(rect(&ui, "card"), Rect::new(0.0, 0.0, 150.0, 98.0));
    assert_eq!(rect(&ui, "a"), Rect::new(12.0, 10.0, 50.0, 30.0));
    assert_eq!(rect(&ui, "b"), Rect::new(12.0, 48.0, 70.0, 40.0));
    assert_eq!(rect(&ui, "c"), Rect::new(90.0, 48.0, 30.0, 20.0));
    let narrow = rect(&ui, "para");
    assert_eq!((narrow.x, narrow.y, narrow.w), (0.0, 98.0, 150.0));

    let lines = |h: f32| { assert_eq!(h % 24.0, 0.0, "height {h} is not whole lines"); (h / 24.0) as u32 };
    let (lw, ln) = (lines(wide.h), lines(narrow.h));
    assert!(lw >= 2 && ln > lw, "wide {lw} lines, narrow {ln} lines");
}
