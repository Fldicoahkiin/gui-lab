//! Acceptance for the annotation layer (DESIGN.md §1.7), without a GPU: the records of a known tree
//! (rects, numbers, sizes, kinds), density, scope and callouts; label packing never overlaps; hover and
//! focus modes give the element and its ancestors; frames draw on and numbers count up.
use amenui::el::{El, Paint, col, row, space, text};
use amenui::gaze::{self, Annot};
use amenui::style::{Gaze, GazeMode, gaze as kinds};
use amenui::ui::{Rect, Ui};

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

struct S { gaze: Gaze }
fn view(_: &S) -> El<()> {
    col().key("root").pad(20.0, 20.0).gap(10.0)
        .child(row().key("bar").width(400.0).height(40.0).fill(Paint::FAINT).child(space(60.0, 20.0).key("chip").fill(Paint::ACCENT)))
        .child(col().key("scope").gaze_scope().width(500.0).pad(10.0, 10.0).gap(8.0)
            .child(space(200.0, 50.0).key("a").fill(Paint::INK).annotate("Power switch"))
            .child(space(100.0, 30.0).key("b").fill(Paint::INK).on_press(()))
            .child(text("说明文字", 14.0, Paint::TEXT).key("c")))
}
fn ui(g: Gaze) -> Ui<S, ()> {
    Ui::new(S { gaze: g }, view, |_, _| {}, fonts(), 800, 600, 1.0).on_gaze(|s| s.gaze)
}
fn all() -> Gaze { Gaze { mode: GazeMode::All, kinds: kinds::FRAME | kinds::INDEX | kinds::SIZE, density: 50.0, draw_ms: 300.0, ..Gaze::default() } }

#[test]
fn records_follow_the_layout() {
    let u = ui(all());
    let a: &[Annot] = u.annots();
    // Only inside the scope: the scope itself is the outermost, then a, b, c in tree order.
    let keys: Vec<&str> = a.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys, ["scope", "a", "b", "c"]);
    assert_eq!(a.iter().map(|r| r.index).collect::<Vec<_>>(), [1, 2, 3, 4]);
    for r in a { assert_eq!(r.rect, u.node(&r.key).unwrap().rect, "{} is framed at its real rect", r.key); }
    let ra = &a[1];
    assert_eq!((ra.rect.w, ra.rect.h), (200.0, 50.0));
    assert_eq!(ra.label.as_deref(), Some("Power switch"));
    assert_eq!(a[2].kind, "control");
    assert_eq!(a[3].kind, "text");
    assert_eq!(ra.kind, "shape");
    // Instances for stage 3 exist, and the page's own scene is untouched by them.
    assert!(!u.scene().annot.is_empty());
}

#[test]
fn density_keeps_the_largest_and_callouts_can_be_the_only_ones() {
    let u = ui(Gaze { density: 2.0, ..all() });
    let keys: Vec<&str> = u.annots().iter().map(|r| r.key.as_str()).collect();
    // A callout comes first, then the largest.
    assert_eq!(keys, ["scope", "a"]);
    let u = ui(Gaze { only_marked: true, ..all() });
    let keys: Vec<&str> = u.annots().iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys, ["a"]);
}

#[test]
fn hover_and_focus_give_the_ancestor_chain() {
    let mut u = ui(Gaze { mode: GazeMode::Hover, ..all() });
    assert!(u.annots().is_empty(), "nothing under no pointer");
    let (x, y) = u.node("b").unwrap().rect.center();
    u.pointer_move(x, y);
    u.tick(0.1);
    let keys: Vec<&str> = u.annots().iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys, ["scope", "b"]);
    // The chain itself, outside the scope rule, reaches the root.
    let i = u.nodes().iter().position(|n| n.key == "b").unwrap();
    let chain: Vec<&str> = gaze::chain(u.nodes(), i).into_iter().map(|k| u.nodes()[k].key.as_str()).collect();
    assert_eq!(chain, ["b", "scope", "root"]);
    let mut u = ui(Gaze { mode: GazeMode::Focus, ..all() });
    u.press(x, y);
    u.tick(0.2);
    let keys: Vec<&str> = u.annots().iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys, ["scope", "b"]);
}

#[test]
fn labels_never_overlap() {
    // Many labels wanting nearly the same place, in a short column.
    let items: Vec<(f32, f32)> = (0..12).map(|i| (100.0 + (i % 3) as f32 * 4.0, 14.0)).collect();
    let tops = gaze::pack(&items, 0.0, 400.0, 6.0);
    let mut spans: Vec<(f32, f32)> = tops.iter().zip(&items).map(|(t, (_, h))| (*t, t + h)).collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    for w in spans.windows(2) { assert!(w[1].0 >= w[0].1 + 6.0 - 1e-3, "{:?} overlaps {:?}", w[0], w[1]); }
    assert!(spans.iter().all(|s| s.0 >= 0.0 && s.1 <= 400.0));
    // Overflowing the bottom pushes them back up.
    let items: Vec<(f32, f32)> = (0..5).map(|_| (390.0, 14.0)).collect();
    let tops = gaze::pack(&items, 0.0, 400.0, 6.0);
    assert!(tops.iter().all(|t| t + 14.0 <= 400.0 + 1e-3));
    // Placed labels, in both columns, inside the scope.
    let annots: Vec<Annot> = (0..8).map(|i| Annot {
        node: i, key: format!("k{i}"), rect: Rect::new(if i % 2 == 0 { 150.0 } else { 600.0 }, 200.0 + i as f32 * 3.0, 40.0, 20.0), index: i as u32 + 1, kind: "shape", label: None,
    }).collect();
    let scope = Rect::new(0.0, 0.0, 800.0, 500.0);
    let boxes = gaze::place_labels(&annots, scope, |_| (80.0, 14.0));
    for (i, (side, b)) in boxes.iter().enumerate() {
        assert_eq!(*side, if i % 2 == 0 { gaze::Side::Left } else { gaze::Side::Right });
        for (j, (_, c)) in boxes.iter().enumerate() {
            if i != j { assert!(b.intersect(c).w * b.intersect(c).h == 0.0, "labels {i} and {j} overlap"); }
        }
    }
}

#[test]
fn frames_draw_on_and_numbers_count_up() {
    assert_eq!(gaze::drawn(0.0, 300.0), 0.0);
    assert_eq!(gaze::drawn(0.15, 300.0), 0.5);
    assert_eq!(gaze::drawn(1.0, 300.0), 1.0);
    assert_eq!(gaze::count(12.0, 0.0), 0.0);
    assert_eq!(gaze::count(12.0, 1.0), 12.0);
    let mut u = ui(all());
    let early = u.scene().annot.len();
    u.tick(1.0);
    // Fully drawn: the frame's corners are at full length; the records do not change.
    assert_eq!(u.annots().len(), 4);
    assert!(u.scene().annot.len() >= early);
}

// ── Edge notes and the field map's readouts (DESIGN.md §4.2, §4.3) ────────────────────────────────
struct R { gaze: Gaze, cmds: Vec<amenui::morph::Cmd> }
fn readout_view(_: &R) -> El<()> {
    col().key("root").pad(20.0, 20.0)
        .child(col().key("scope").gaze_scope().width(520.0).height(420.0).pad(16.0, 16.0).gap(10.0)
            .child(space(200.0, 60.0).key("deco").fill(Paint::INK).respond(amenui::el::Response::Loosen))
            .child(text("说明文字", 14.0, Paint::TEXT).key("c")))
}
fn readouts(kinds: u16) -> Ui<R, ()> {
    let g = Gaze { mode: GazeMode::All, kinds, density: 10.0, draw_ms: 0.0, ..Gaze::default() };
    let mut u = Ui::new(R { gaze: g, cmds: vec![] }, readout_view, |_, _| {}, fonts(), 800, 600, 1.0)
        .on_gaze(|s| s.gaze).on_commands(|s| std::mem::take(&mut s.cmds))
        .on_look(|_| amenui::style::Look { material: amenui::style::Material::Plain, ..amenui::style::Look::default() });
    u.pointer_leave();
    u.tick(1.0);
    u
}
fn stir(u: &mut Ui<R, ()>, x: f32, y: f32) {
    u.state.cmds.push(amenui::morph::Cmd::Stir { at: amenui::morph::StirAt::Point([x, y]), v: [1.0, 0.0], energy: 1.0, radius: 40.0, scope: amenui::el::Scope::Global });
    u.relayout();
}

#[test]
fn edge_notes_say_what_is_true_now() {
    let mut u = readouts(kinds::EDGE);
    let sc = u.node("scope").unwrap().rect;
    let r = u.readout().clone();
    assert_eq!(r.scope, sc);
    let texts: Vec<&str> = r.notes.iter().map(|n| n.0.as_str()).collect();
    assert_eq!(texts, [format!("{:.0} × {:.0}", sc.w, sc.h).as_str(), "x — y —", "plain · dark", "loose 0.00", "0 nodes"]);
    // The hairline is on the scope's right edge, one device pixel inside it.
    assert_eq!(r.edge_x, Some(sc.right() - 1.0));
    // Every note inside the scope, none on another.
    for (i, (_, a)) in r.notes.iter().enumerate() {
        assert!(a.x >= sc.x && a.y >= sc.y && a.right() <= sc.right() && a.bottom() <= sc.bottom(), "note {i} at {a:?} is outside {sc:?}");
        for (_, b) in &r.notes[i + 1..] { let o = a.intersect(b); assert!(o.w <= 0.0 || o.h <= 0.0, "{a:?} overlaps {b:?}"); }
    }
    // The pointer in the scope; a stir: the notes follow.
    u.pointer_move(sc.x + 100.0, sc.y + 50.0);
    stir(&mut u, sc.x + 120.0, sc.y + 40.0);
    u.tick(1.05);
    let r = u.readout().clone();
    let now = u.now();
    assert_eq!(r.notes[1].0, "x 100 y 50");
    assert_eq!(r.mark_y, Some(sc.y + 50.0));
    assert_eq!(r.notes[3].0, format!("loose {:.2}", u.looseness("root", now)));
    assert!(u.looseness("root", now) > 0.0);
    assert_eq!(r.notes[4].0, format!("{} nodes", u.fieldmap().nonzero()));
    assert!(u.fieldmap().nonzero() > 0);
}

#[test]
fn the_waveform_and_the_level_read_the_field_map() {
    let mut u = readouts(kinds::OSC | kinds::LEVEL);
    let g = Gaze::default();
    let sc = u.node("scope").unwrap().rect;
    // Nothing in the map: a straight line, no level, and no frames for two seconds.
    let r = u.readout().clone();
    assert!(!r.osc.is_empty());
    let base = sc.y + sc.h * g.osc_y;
    assert!(r.osc.iter().all(|p| p[1] == base), "flat at {base}: {:?}", &r.osc[..4]);
    assert_eq!((r.level, r.level_h), (0.0, 0.0));
    let (mut t, mut drawn) = (1.0, 0);
    u.frame_drawn();
    while t < 3.0 { t += 1.0 / 60.0; u.tick(t); if u.needs_frame(t) { drawn += 1; } u.frame_drawn(); }
    assert_eq!(drawn, 0, "a still map asks for no frames");
    // A stir at x: the highest point is within a cell of x, as high as the energy there × osc_amp.
    let (x, y) = (sc.x + 300.0, sc.y + 200.0);
    stir(&mut u, x, y);
    u.tick(t + 1.0 / 60.0);
    let r = u.readout().clone();
    let m = u.fieldmap();
    let top = r.osc.iter().copied().fold([0.0, f32::MAX], |a, p| if p[1] < a[1] { p } else { a });
    assert!((top[0] - x).abs() <= m.cell, "peak at {} for a stir at {x}", top[0]);
    let i = (top[0] / m.cell).round() as usize;
    assert!(((base - top[1]) - m.byte_e(i, r.osc_row) * g.osc_amp).abs() <= 1.0);
    assert!(r.osc_row.abs_diff((y / m.cell).round() as usize) <= 1, "the strongest row {} for a stir at {y}", r.osc_row);
    // The level is the formula, and what the tree exports.
    assert!(r.level > 0.0);
    assert_eq!(r.level_h, gaze::level_height(sc.h, m.level(), g.level_full));
    assert_eq!(u.export_tree().field.map(|f| f.level), Some(r.level));
    // Settled: flat again.
    t = u.now();
    while u.fieldmap().nonzero() > 0 && t < 30.0 { t += 1.0 / 30.0; u.tick(t); }
    u.tick(t + 1.0 / 30.0);
    let r = u.readout().clone();
    assert!(r.osc.iter().all(|p| p[1] == base) && r.level_h == 0.0, "settled by {t}");
}

// Labels never cover text to read, nor each other: the pages the audit breaks on purpose, every element
// numbered and named, with its true ink measured.
struct B { gaze: Gaze, page: u8 }
fn broken(b: &B) -> El<()> {
    let line = "two lines drawn over each other";
    let body = match b.page {
        0 => col().child(text("this line runs on well past the right edge of the window and is cut off there", 22.0, Paint::TEXT).nowrap().key("long")),
        1 => col().child(text("grey text on a grey ground", 18.0, Paint::tone(0.14)).key("grey")),
        2 => amenui::el::layers([text(line, 18.0, Paint::TEXT).key("under"), text(line, 18.0, Paint::TEXT).key("over").shift(2.0, 3.0)]),
        3 => col().child(text("six pixels tall", 6.0, Paint::TEXT).key("tiny")),
        _ => col().gap(8.0).child(text("This is a calm page", 22.0, Paint::INK).key("title")).child(text("Everything on it reads, and the next line is close.", 15.0, Paint::DIM).key("note")),
    };
    col().key("page").pad(20.0, 20.0).child(body)
}
#[test]
fn labels_cover_no_text_to_read_and_no_other_label() {
    let g = Gaze { mode: GazeMode::All, kinds: kinds::FRAME | kinds::INDEX | kinds::KIND | kinds::INK, density: 100.0, draw_ms: 0.0, ..Gaze::default() };
    for page in 0..5u8 {
        let mut u = Ui::new(B { gaze: g, page }, broken, |_, _| {}, fonts(), 480, 240, 1.0).on_gaze(|s| s.gaze);
        u.tick(1.0);
        let inks: Vec<[f32; 4]> = u.glyphs().log.iter().filter(|gl| u.nodes()[gl.node].legible)
            .map(|gl| amenui::form::ink_box(&u.scene().instances[gl.inst])).filter(|b| b[2] > b[0] && b[3] > b[1]).collect();
        // A label's plate: a shape in the page's ground under its text.
        let plates: Vec<[f32; 4]> = u.scene().annot.iter().filter(|i| i.paint[1] == 1.0 && i.paint[3] == 1.0).map(|i| [i.rect[0], i.rect[1], i.rect[0] + i.rect[2], i.rect[1] + i.rect[3]]).collect();
        assert!(!plates.is_empty(), "page {page}: some label is drawn");
        let meet = |a: &[f32; 4], b: &[f32; 4]| (a[2].min(b[2]) - a[0].max(b[0])) > 0.5 && (a[3].min(b[3]) - a[1].max(b[1])) > 0.5;
        for (k, p) in plates.iter().enumerate() {
            for i in &inks { assert!(!meet(p, i), "page {page}: a label {p:?} covers text {i:?}"); }
            for q in &plates[k + 1..] { assert!(!meet(p, q), "page {page}: labels {p:?} and {q:?} meet"); }
        }
    }
}

// The INK leader: dashed, its free end and every dash 12 px clear of other elements' text to read, its
// other end on the element's true outline.
struct L { gaze: Gaze }
fn leader_page(_: &L) -> El<()> {
    let mut d = amenui::draw::Drawing::new(120.0, 120.0);
    d.dot(60.0, 60.0, 50.0, Paint::ACCENT);
    col().key("page").pad(20.0, 20.0)
        .child(amenui::el::canvas(d).key("disc").width(120.0).height(120.0).absolute(Some(60.0), Some(70.0), None, None))
        .child(text("A caption over the corner", 13.0, Paint::TEXT).key("caption").absolute(Some(20.0), Some(48.0), None, None))
        .child(text("Notes on the right", 15.0, Paint::TEXT).key("notes").absolute(Some(240.0), Some(110.0), None, None))
}
#[test]
fn an_ink_leader_keeps_off_text_to_read_and_ends_on_the_outline() {
    let g = Gaze { mode: GazeMode::All, kinds: kinds::INK, density: 100.0, draw_ms: 300.0, ..Gaze::default() };
    let mut u = Ui::new(L { gaze: g }, leader_page, |_, _| {}, fonts(), 480, 260, 1.0).on_gaze(|s| s.gaze);
    u.tick(2.0);
    let mut inks: Vec<(String, [f32; 4])> = vec![];
    for gl in &u.glyphs().log {
        let n = &u.nodes()[gl.node];
        if !n.legible { continue; }
        let b = amenui::form::ink_box(&u.scene().instances[gl.inst]);
        match inks.iter_mut().find(|e| e.0 == n.key) {
            Some(e) => e.1 = [e.1[0].min(b[0]), e.1[1].min(b[1]), e.1[2].max(b[2]), e.1[3].max(b[3])],
            None => inks.push((n.key.clone(), b)),
        }
    }
    let leaders = u.leaders().to_vec();
    let disc = leaders.iter().find(|l| l.0 == "disc").expect("the disc has a leader");
    // Its far end is on the circle the disc draws (centre 120, 130; radius 50).
    let e = *disc.1.last().expect("a path");
    let off = ((e[0] - 120.0).hypot(e[1] - 130.0) - 50.0).abs();
    assert!(off < 1.0, "the leader ends {off:.2} px off the outline ({e:?})");
    // The corner by the caption is not clear: it starts at another one.
    let s0 = disc.1[0];
    assert!((s0[0] - 60.0).abs() > 1.0 || (s0[1] - 70.0).abs() > 1.0, "it does not start by the caption: {s0:?}");
    // It starts on a corner of the disc's ink box (70, 80 to 170, 180) and stays inside the box.
    let corner = [[70.0f32, 80.0], [170.0, 80.0], [170.0, 180.0], [70.0, 180.0]].into_iter().map(|c| (c[0] - s0[0]).hypot(c[1] - s0[1])).fold(f32::MAX, f32::min);
    assert!(corner < 1.0, "the leader starts {corner:.2} px from the box's corners ({s0:?})");
    for q in &disc.1 { assert!(q[0] >= 69.5 && q[0] <= 170.5 && q[1] >= 79.5 && q[1] <= 180.5, "the leader leaves the box at {q:?}"); }
    // None is shorter than two dashes: the text's boxes hug their ink and have none.
    for (key, path) in &leaders {
        let len: f32 = path.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum();
        assert!(len >= 16.0, "{key}'s leader is {len:.1} px");
    }
    assert!(leaders.iter().all(|l| l.0 == "disc"), "only the disc has a leader: {:?}", leaders.iter().map(|l| &l.0).collect::<Vec<_>>());
    // And it ends on the near side of the disc from where it starts, not across it.
    let reach = (s0[0] - 120.0).hypot(s0[1] - 130.0) - 50.0;
    let run = (e[0] - s0[0]).hypot(e[1] - s0[1]);
    assert!(run < reach + 1.5, "the leader runs {run:.1} px where the outline is {reach:.1} px from its start");
    for (key, path) in &leaders {
        for (other, b) in inks.iter().filter(|(k, _)| k != key) {
            let g = [b[0] - 12.0, b[1] - 12.0, b[2] + 12.0, b[3] + 12.0];
            for w in path.windows(2) {
                let n = ((w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]) * 2.0).ceil().max(1.0) as usize;
                for k in 0..=n {
                    let u = k as f32 / n as f32;
                    let p = [w[0][0] + (w[1][0] - w[0][0]) * u, w[0][1] + (w[1][1] - w[0][1]) * u];
                    assert!(!(p[0] > g[0] && p[0] < g[2] && p[1] > g[1] && p[1] < g[3]), "{key}'s leader passes within 12 px of {other} at {p:?}");
                }
            }
        }
    }
    // Drawn on: the dashes already drawn stay where they are, frame to frame.
    let mut u = Ui::new(L { gaze: g }, leader_page, |_, _| {}, fonts(), 480, 260, 1.0).on_gaze(|s| s.gaze);
    let mut t = 1.0;
    u.tick(t);
    let mut last: Vec<[f32; 4]> = vec![];
    for _ in 0..20 {
        t += 1.0 / 60.0;
        u.tick(t);
        let now: Vec<[f32; 4]> = u.scene().annot.iter().filter(|i| i.xf != [0.0; 4]).map(|i| i.rect).collect();
        assert!(last.iter().all(|r| now.contains(r)), "a drawn dash moved");
        last = now;
    }
}

// Echoes of the INK box: a block that moves, on a beat of 120 bpm, four steps a beat.
struct E { gaze: Gaze, x: f32 }
fn echo_page(s: &E) -> El<()> {
    col().key("page").pad(20.0, 20.0).child(space(60.0, 40.0).key("block").fill(Paint::INK).absolute(Some(s.x), Some(60.0), None, None))
}
fn echo_look(_: &E) -> amenui::style::Look { amenui::style::Look { material: amenui::style::Material::Plain, beat: amenui::beat::Beat { bpm: 120.0, steps: 4, swing: 0.0 }, ..amenui::style::Look::default() } }
/// The echo boxes drawn: the faint outlines (4 bars each) that are not the INK box itself.
fn echo_bars(u: &Ui<E, ()>) -> usize { u.scene().annot.iter().filter(|i| i.paint[3] == 1.0 && i.paint[1] == 0.0 && i.paint[2] <= 0.6 + 1e-4 && i.xf == [0.0; 4]).count() }
#[test]
fn echoes_keep_at_most_three_boxes_on_the_grid_and_then_ask_for_nothing() {
    let g = Gaze { mode: GazeMode::All, kinds: kinds::INK, density: 10.0, draw_ms: 0.0, echo: 3, echo_ms: 200.0, ..Gaze::default() };
    let mut u = Ui::new(E { gaze: g, x: 40.0 }, echo_page, |_, _| {}, fonts(), 480, 200, 1.0).on_gaze(|s| s.gaze).on_look(echo_look);
    let mut t = 0.0;
    let mut most = 0;
    // It moves for two seconds.
    for _ in 0..120 {
        t += 1.0 / 60.0;
        u.state.x += 2.0;
        u.relayout();
        u.tick(t);
        u.frame_drawn();
        most = most.max(echo_bars(&u) / 4);
        assert_eq!(echo_bars(&u) % 4, 0);
    }
    assert!((2..=3).contains(&most), "at most three echoes, and some: {most}");
    // Each echo was taken on the grid: two steps (0.25 s) apart at 120 bpm, 4 steps a beat.
    let times = u.echo_times("block");
    assert!(!times.is_empty());
    for w in times.windows(2) { let d = w[1] - w[0]; assert!((d / 0.25 - (d / 0.25).round()).abs() < 1e-3 && d > 0.2, "echoes {times:?}"); }
    for t0 in &times { assert!((t0 / 0.125 - (t0 / 0.125).round()).abs() < 1e-3, "{t0} is not on a step"); }
    // It stops: the echoes fade, and two seconds after they have, no frames.
    let mut end = t;
    for _ in 0..240 { t += 1.0 / 60.0; u.tick(t); if u.needs_frame(t) { end = t; } u.frame_drawn(); }
    assert_eq!(echo_bars(&u), 0, "faded");
    assert!(t - end >= 2.0, "frames were asked until {end}, {t} now");
}

#[test]
fn a_hold_takes_no_echo_and_fades_none() {
    let Ok(mut o) = amenui::offscreen::Offscreen::try_new(480, 200) else { eprintln!("no GPU adapter here: skipped"); return };
    let g = Gaze { mode: GazeMode::All, kinds: kinds::INK, density: 10.0, draw_ms: 0.0, echo: 3, echo_ms: 200.0, ..Gaze::default() };
    let mut u = Ui::new(E { gaze: g, x: 40.0 }, echo_page, |_, _| {}, fonts(), 480, 200, 1.0).on_gaze(|s| s.gaze).on_look(echo_look);
    let mut t = 0.0;
    for _ in 0..60 { t += 1.0 / 60.0; u.state.x += 2.0; u.relayout(); u.tick(t); let l = u.look(); let _ = o.render(&mut u, &l); }
    assert!(echo_bars(&u) > 0, "echoes are showing");
    u.command(amenui::morph::Cmd::Hold(2));
    let mut frames = vec![];
    // Two steps at 120 bpm, 4 a beat: 0.25 s; the frames inside it, the block still moving in the
    // state all the while: what is presented does not move.
    for _ in 0..12 { t += 1.0 / 60.0; u.state.x += 2.0; u.relayout(); u.tick(t); let l = u.look(); frames.push(o.render(&mut u, &l)); }
    assert!(frames.windows(2).all(|w| w[0] == w[1]), "the frames of a hold are the same bytes");
    // After it the page is the state's again.
    for _ in 0..6 { t += 1.0 / 60.0; u.tick(t); }
    let l = u.look();
    assert!(o.render(&mut u, &l) != frames[0], "after the hold the block is drawn where the state has it");
}

#[test]
fn the_newest_echo_is_at_most_one_interval_old_and_the_three_step_down_in_contrast() {
    let Ok(mut o) = amenui::offscreen::Offscreen::try_new(480, 200) else { eprintln!("no GPU adapter here: skipped"); return };
    let g = Gaze { mode: GazeMode::All, kinds: kinds::INK, density: 10.0, draw_ms: 0.0, echo: 3, echo_ms: 200.0, ..Gaze::default() };
    let mut u = Ui::new(E { gaze: g, x: 20.0 }, echo_page, |_, _| {}, fonts(), 480, 200, 1.0).on_gaze(|s| s.gaze).on_look(echo_look);
    let mut t = 0.0;
    let mut px = vec![];
    for _ in 0..80 { t += 1.0 / 60.0; u.state.x += 3.0; u.relayout(); u.tick(t); let l = u.look(); px = o.render(&mut u, &l); }
    // 200 ms is two steps of 0.125 s at 120 bpm, 4 a beat: 0.25 s.
    let times = u.echo_times("block");
    let newest = times.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    assert!(u.beat_grid() - newest <= 0.25 + 1e-3, "the newest echo was taken at {newest}, now {}", u.beat_grid());
    // The echo outlines (thin, faint bars), by opacity: each against the ground.
    let ground = { let i = (10 * 480 + 470) * 4; [px[i] as f32 / 255.0, px[i + 1] as f32 / 255.0, px[i + 2] as f32 / 255.0] };
    let lin = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    let lum = |c: [f32; 3]| 0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2]);
    let ratio = |a: [f32; 3]| { let (x, y) = (lum(a), lum(ground)); (x.max(y) + 0.05) / (x.min(y) + 0.05) };
    let mut seen: Vec<(f32, f32)> = vec![];
    for i in u.scene().annot.iter().filter(|i| i.paint[3] == 1.0 && i.paint[1] == 0.0 && i.paint[2] < 0.6 && i.xf == [0.0; 4] && i.rect[2] <= 2.0) {
        let (x, y) = ((i.rect[0] + 0.5) as usize, (i.rect[1] + i.rect[3] * 0.5) as usize);
        let k = (y * 480 + x) * 4;
        let c = [px[k] as f32 / 255.0, px[k + 1] as f32 / 255.0, px[k + 2] as f32 / 255.0];
        if !seen.iter().any(|s| (s.0 - i.paint[2]).abs() < 1e-4) { seen.push((i.paint[2], ratio(c))); }
    }
    seen.sort_by(|a, b| b.0.total_cmp(&a.0));
    eprintln!("echoes (opacity, contrast): {seen:?}");
    assert_eq!(seen.len(), 3, "three echoes: {seen:?}");
    for ((op, c), want) in seen.iter().zip([2.2f32, 1.6, 1.25]) { assert!(*c >= want, "the echo at {op} has {c:.2}:1, under {want}:1"); }
    let ops: Vec<f32> = seen.iter().map(|s| s.0 / 0.9).collect();
    for (o, want) in ops.iter().zip([0.6f32, 0.4, 0.2]) { assert!((o - want).abs() < 1e-3, "opacities {ops:?}"); }
}

#[test]
fn the_annotation_colour_reads_on_both_themes() {
    // The INK label at least 4.5:1 against the ground and the dashes at least 3:1, dark and light.
    let Ok(mut o) = amenui::offscreen::Offscreen::try_new(480, 260) else { eprintln!("no GPU adapter here: skipped"); return };
    struct T { gaze: Gaze, theme: usize }
    fn page(_: &T) -> El<()> { leader_page(&L { gaze: Gaze::default() }) }
    let lin = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    let lum = |c: [f32; 3]| 0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2]);
    let ratio = |a: [f32; 3], b: [f32; 3]| { let (x, y) = (lum(a), lum(b)); (x.max(y) + 0.05) / (x.min(y) + 0.05) };
    for theme in [0usize, 1] {
        let g = Gaze { mode: GazeMode::All, kinds: kinds::INK, density: 100.0, draw_ms: 0.0, ..Gaze::default() };
        let mut u = Ui::new(T { gaze: g, theme }, page, |_, _| {}, fonts(), 480, 260, 1.0).on_gaze(|s| s.gaze)
            .on_look(|s| amenui::style::Look { material: amenui::style::Material::Plain, theme: amenui::style::THEMES[s.theme], ..amenui::style::Look::default() });
        u.tick(2.0);
        let l = u.look();
        let px = o.render(&mut u, &l);
        let ground = l.theme.ground;
        let at = |x: f32, y: f32| { let i = ((y.max(0.0) as usize).min(259) * 480 + (x.max(0.0) as usize).min(479)) * 4; [px[i] as f32 / 255.0, px[i + 1] as f32 / 255.0, px[i + 2] as f32 / 255.0] };
        // The label: the strongest pixel of its glyphs.
        let mut label = 0.0f32;
        for i in u.scene().annot.iter().filter(|i| i.paint[3] == amenui::scene::GLYPH) {
            let r = amenui::form::ink_box(i);
            let (mut y, mut best) = (r[1].floor(), 0.0f32);
            while y < r[3] { let mut x = r[0].floor(); while x < r[2] { best = best.max(ratio(at(x + 0.5, y + 0.5), ground)); x += 1.0; } y += 1.0; }
            label = label.max(best);
        }
        assert!(label >= 4.5, "theme {theme}: the INK label is {label:.2}:1");
        // A dash of the box: the middle of each short bar.
        let mut dash = f32::MAX;
        for i in u.scene().annot.iter().filter(|i| i.paint[3] == amenui::scene::SHAPE && i.paint[1] == 0.0 && i.xf == [0.0; 4] && i.paint[2] > 0.8) {
            dash = dash.min(ratio(at(i.rect[0] + i.rect[2] * 0.5, i.rect[1] + i.rect[3] * 0.5), ground));
        }
        assert!(dash >= 3.0, "theme {theme}: a dash is {dash:.2}:1");
    }
}
