//! A front pushes text to read and controls as one piece with the control they are in, measured at that
//! control's centre. A surface that holds them (a stage that follows the pointer or takes keys, or any
//! element larger than a quarter of the window) is not a piece: each control in it is its own.
use amenui::el::{El, Key, layers, row};
use amenui::ui::Ui;
use amenui::widgets::button;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

#[derive(Clone, Copy)]
struct S { point: bool, keys: bool }
#[derive(Clone, Debug, PartialEq)]
enum M { A, B, Any }
fn view(s: &S) -> El<M> {
    let buttons = row().key("row").gap(400.0).pad(40.0, 40.0).child(button("a", "Apply", M::A)).child(button("b", "Bring", M::B));
    let mut stage = layers(vec![buttons]).key("stage").fill_width().fill_height();
    if s.point { stage = stage.on_point(|_| Some(M::Any)); }
    if s.keys { stage = stage.on_key(|k: Key, _| (k == Key::Enter).then_some(M::Any)); }
    stage
}

/// The centres (device px) the instances of `key`'s subtree are pushed about.
fn centres(ui: &Ui<S, M>, key: &str) -> Vec<[f32; 2]> {
    let mut v: Vec<[f32; 2]> = ui.scene().instances.iter().enumerate()
        .filter(|(i, inst)| inst.resp[1] > 1.5 && ui.owner_key(*i).is_some_and(|k| k.starts_with(key)))
        .map(|(_, inst)| [inst.resp[2], inst.resp[3]]).collect();
    v.dedup();
    v
}

#[test]
fn a_stage_round_controls_is_not_their_piece() {
    for (point, keys) in [(true, false), (false, true), (true, true)] {
        let mut ui = Ui::new(S { point, keys }, view, |_, _| {}, fonts(), 1200, 600, 2.0);
        ui.tick(0.5);
        for k in ["a", "b"] {
            let c = ui.node(k).expect("button").rect.center();
            let got = centres(&ui, k);
            assert!(!got.is_empty(), "{k}: nothing pushed as a piece");
            for g in &got {
                assert!((g[0] - c.0 * 2.0).abs() < 0.5 && (g[1] - c.1 * 2.0).abs() < 0.5,
                    "point {point} keys {keys}: {k} is pushed about {g:?}, not its own centre {:?} (the stage's is [600, 300])", [c.0 * 2.0, c.1 * 2.0]);
            }
        }
    }
}
