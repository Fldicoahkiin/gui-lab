//! A morph from text posed inside a stage (DESIGN.md §1.10, §1.35): the stage steps by itself, its glyphs
//! posed anew each step, and a morph from them starts its tiles where they were last drawn.
use amenui::el::{El, Paint, Pose, col, text};
use amenui::style::{Look, Material};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

#[derive(Clone, Default)]
struct S { t: f32, at: Option<f32>, hide: bool, cmds: Vec<amenui::morph::Cmd> }
fn piece(s: &S) -> El<()> {
    let (t, hide) = (s.t, s.hide);
    col().key("piece").stage().width(300.0).height(160.0)
        .child(text("a stone of words", 18.0, Paint::TEXT).key("stone").pose(move |g| Pose { dx: 40.0 * (t * 1.7 + g.i as f32 * 0.3).sin(), dy: 20.0 * (t * 1.1).cos(), ..Pose::default() }).hidden_until_morphed(hide))
}
fn view(s: &S) -> El<()> {
    col().key("page").pad(20.0, 20.0).gap(20.0).child(piece(s)).child(text("word", 28.0, Paint::TEXT).key("word").hidden_until_morphed(true))
}

#[test]
fn a_morph_from_text_posed_in_a_stage_starts_where_its_glyphs_were_drawn() {
    let mut u = Ui::new(S::default(), view, |_, _| {}, fonts(), 480, 320, 1.0).on_look(|_| Look { material: Material::Plain, ..Look::default() })
        .on_step(|s, t| { s.t = t; vec!["piece".to_owned()] })
        .stage_view(|s, k| (k == "piece").then(|| piece(s)));
    let mut t = 1.0;
    u.tick(t);
    u.settle();
    let restaged = u.perf().counters.restages;
    for _ in 0..20 { t += 1.0 / 60.0; u.tick(t); u.settle(); u.frame_drawn(); }
    assert!(u.perf().counters.restages > restaged + 10, "the stage steps by itself");
    // The stone's glyphs as drawn in the last frame.
    let drawn: Vec<[f32; 4]> = (0..u.scene().instances.len()).filter(|&i| u.owner_key(i) == Some("stone")).map(|i| u.scene().instances[i].rect).collect();
    assert!(drawn.len() > 10, "the stone's glyphs: {}", drawn.len());
    u.morph("stone", "word", amenui::morph::MorphSpec::default());
    let run = u.morph_run().expect("a morph");
    let src: Vec<[f32; 4]> = run.src.iter().map(|i| i.rect).collect();
    assert_eq!(src, drawn, "the morph's source is the glyphs as they were drawn");
}

/// The site's way: a tick changes the state (the pose moves on with it) and asks for the morph by a
/// command, so the view is laid out and drawn again before the morph takes its source, and the new view
/// hides the stone until it has gone (as the site's does). The tiles still start where the glyphs were
/// in the frame drawn last, not where the new view would pose them.
#[test]
fn a_morph_asked_for_by_a_command_as_the_pose_moves_on_starts_where_the_glyphs_were_drawn() {
    // The morph is asked for on the tick after the 20th frame.
    let st = S { at: Some(1.0 + 20.5 / 60.0), ..S::default() };
    let mut u = Ui::new(st, view, |_, _| {}, fonts(), 480, 320, 1.0).on_look(|_| Look { material: Material::Plain, ..Look::default() })
        .on_tick(|s, t| {
            if s.at.is_some_and(|a| t >= a) {
                s.at = None;
                (s.t, s.hide) = (t, true);
                s.cmds.push(amenui::morph::Cmd::Morph { from: "stone".into(), to: "word".into(), spec: amenui::morph::MorphSpec::default() });
                return true;
            }
            false
        })
        .on_step(|s, t| { s.t = t; vec!["piece".to_owned()] })
        .on_commands(|s| std::mem::take(&mut s.cmds))
        .stage_view(|s, k| (k == "piece").then(|| piece(s)));
    let mut t = 1.0;
    u.tick(t);
    u.settle();
    for _ in 0..20 { t += 1.0 / 60.0; u.tick(t); u.settle(); u.frame_drawn(); }
    let drawn: Vec<[f32; 4]> = (0..u.scene().instances.len()).filter(|&i| u.owner_key(i) == Some("stone")).map(|i| u.scene().instances[i].rect).collect();
    assert!(drawn.len() > 10, "the stone's glyphs: {}", drawn.len());
    assert!(u.morph_run().is_none(), "no morph yet");
    t += 1.0 / 60.0;
    u.tick(t);
    u.settle();
    let run = u.morph_run().expect("a morph");
    let src: Vec<[f32; 4]> = run.src.iter().map(|i| i.rect).collect();
    assert_eq!(src, drawn, "the morph's source is the glyphs as the last frame drew them");
}
