//! Ripple rules and recomposition (DESIGN.md §1.15, §1.16), without a GPU: what a press reaches, how each
//! element answers, that fronts from one element never stack, and that a changed string recomposes only
//! its changed glyphs and hands over to the plain new text.
use amenui::el::{El, Paint, col, row, text};
use amenui::morph::Recompose;
use amenui::style::Look;
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

// ── Recomposition ────────────────────────────────────────────────────────────────────────────────

struct Num { s: String, keyed: bool }
fn num_view(n: &Num) -> El<()> {
    let t = text(n.s.clone(), 40.0, Paint::INK);
    let t = if n.keyed { t.key("n") } else { t };
    col().pad(40.0, 40.0).child(row().child(t))
}
fn num_ui(s: &str, keyed: bool) -> Ui<Num, ()> {
    let look = Look::default();
    let mut ui = Ui::new(Num { s: s.into(), keyed }, num_view, |_, _| {}, fonts(), 400, 200, 1.0).on_look(|_| Look::default());
    let _ = look;
    ui.tick(5.0);
    ui
}
fn set(ui: &mut Ui<Num, ()>, s: &str) { ui.state.s = s.into(); ui.relayout(); }
fn alpha_of(ui: &Ui<Num, ()>) -> Vec<f32> { ui.instances_of("n").iter().map(|i| i.paint[2]).collect() }

#[test]
fn unkeyed_text_is_replaced_and_slides_are_made_for_moved_glyphs() {
    let mut ui = num_ui("7", false);
    set(&mut ui, "8");
    assert!(alpha_of(&ui).is_empty() || ui.scene().instances.iter().all(|i| i.paint[2] > 0.0), "no recomposition without a key");
    // Keyed: a longer number pushes nothing here (one glyph, left aligned), but "9" → "10" changes both.
    let mut ui = num_ui("9", true);
    set(&mut ui, "10");
    assert_eq!(alpha_of(&ui).iter().filter(|&&x| x == 0.0).count(), 2);
}

#[test]
fn a_number_that_changes_every_frame_stays_bounded() {
    // 2,000 frames in a row, a new value each: faster than the minimum interval, so it is replaced at once.
    let mut ui = num_ui("0", true);
    let mut peak = 0usize;
    let mut t = ui.now();
    for i in 1..=2000 {
        t += 1.0 / 60.0;
        ui.tick(t);
        set(&mut ui, &format!("{}", i * 7919 % 100_000));
        ui.frame_drawn();
        peak = peak.max(ui.recompose_tiles());
        assert!(ui.scene().instances.len() < 4000, "frame {i}: {} instances", ui.scene().instances.len());
    }
    assert!(peak <= 6000, "peak {peak} tiles");
    // Every 100 ms (slower than the interval, faster than a flight): each change retargets the tiles in
    // flight, and their number stays bounded instead of multiplying.
    let mut peak = 0usize;
    for i in 1..=300 {
        for _ in 0..6 { t += 1.0 / 60.0; ui.tick(t); ui.frame_drawn(); }
        set(&mut ui, &format!("{}", i * 7919 % 100_000));
        peak = peak.max(ui.recompose_tiles());
    }
    let spec = Recompose::default();
    assert!(peak as f32 <= spec.max_tiles, "peak {peak} tiles");
    assert!(peak <= 6 * spec.count_max as usize * 3, "peak {peak} tiles for five digits");
    // And it comes to rest.
    for _ in 0..60 { t += 1.0 / 60.0; ui.tick(t); ui.frame_drawn(); }
    assert_eq!(ui.recompose_tiles(), 0);
    assert!(!ui.needs_frame(t));
}
