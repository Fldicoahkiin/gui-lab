//! The frame budget (DESIGN.md §1.27): however many marks a page asks for, what is made and drawn in
//! a frame stays bounded, and so does the time a frame takes on the CPU.
use lab::draw::Drawing;
use lab::el::{El, Paint, canvas, col, space};
use lab::motes::FlowField;

fn fonts() -> Vec<lab::text::Face> { lab::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

struct S { t: f32 }
fn view(s: &S) -> El<()> {
    // An absurd drawing (a million dots) and an absurd flow field (a mark per 0.01 px²).
    let mut d = Drawing::new(400.0, 300.0);
    for i in 0..1_000_000u32 { d.dot((i % 1000) as f32 * 0.4, (i / 1000) as f32 * 0.3, 0.2, Paint::INK); }
    col().child(canvas(d).key("many").width(400.0))
        .child(space(400.0, 300.0).key("ground").flow_field(FlowField { density: 1.0e6, speed: 10.0 + s.t, ..FlowField::default() }))
}

#[test]
fn absurd_counts_stay_bounded() {
    lab::budget::set(lab::budget::MAX_MARKS, 1.0);
    let mut ui = lab::ui::Ui::new(S { t: 0.0 }, view, |_, _| {}, fonts(), 800, 800, 1.0).on_tick(|s, t| { s.t = t; false });
    let mut worst = 0.0f64;
    let mut t = 0.0;
    for _ in 0..30 {
        t += 1.0 / 60.0;
        let t0 = std::time::Instant::now();
        ui.tick(t);
        worst = worst.max(t0.elapsed().as_secs_f64() * 1000.0);
    }
    let n = ui.scene().instances.len();
    // Every mark the frame shows came out of the budget: the drawing at most `cap`, the field what was granted.
    assert!(n <= 2 * lab::budget::MAX_MARKS + 64, "{n} instances");
    assert!(lab::budget::usage().asked > 1_000_000, "the field asked for its absurd count");
    // A frame of the scene (no GPU here) stays well under a second even in a debug build's worst case.
    assert!(worst < 1000.0, "{worst:.1} ms");
    eprintln!("worst tick {worst:.1} ms, {n} instances");
}
