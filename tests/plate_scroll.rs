//! A cached subtree (`El::cached`) in a scroll area moves with it (DESIGN.md §1.33): a scroll step moves
//! the plate's images with the last frame and inks only the band scrolled into view and the boxes of
//! what changed in it, and the frame is the frame drawn whole, byte for byte, as is what a view of the
//! plate (`el::cached_view`) over the page shows of it. A renderer made after it draws the same.
use amenui::el::{El, Paint, cached_view, col, row, space, text};
use amenui::offscreen::Offscreen;
use amenui::style::{Look, Material};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
const W: u32 = 420;
const H: u32 = 320;

struct S { mat: Material, n: u32 }
fn view(s: &S) -> El<()> {
    let page = (0..50).fold(col().key("sheet").gap(4.0).pad(12.0, 12.0), |c, i| {
        let words = if i == 20 { format!("Line {i}, read {} times", s.n) } else { format!("Line {i}: small print kept on a plate as it scrolls") };
        c.child(row().key(format!("line{i}")).gap(8.0).child(space(10.0, 10.0).radius(2.0).fill(if i % 4 == 0 { Paint::ACCENT } else { Paint::DIM }))
            .child(text(words, 11.0, Paint::TEXT)))
    });
    let list = col().key("list").fill_width().grow(1.0).scroll()
        .child(text("Above the sheet", 14.0, Paint::INK).key("above"))
        .child(page.cached())
        .child(text("Below the sheet", 14.0, Paint::INK).key("below"));
    col().key("page").fill_width().fill_height()
        .child(text("A bar over the list that stays", 16.0, Paint::INK).key("bar"))
        .child(list)
        // A view of the plate over the page: what it shows is what the plate holds where the list is.
        .child(col().key("glass").width(80.0).height(80.0).clip_round(40.0).absolute(Some(320.0), Some(8.0), None, None)
            .child(cached_view("sheet", [40.0, 120.0, 40.0, 40.0], Paint::TEXT).width(80.0).height(80.0)))
}
fn open(mat: Material, scale: f32) -> Ui<S, ()> {
    let mut u = Ui::new(S { mat, n: 0 }, view, |_, _| {}, fonts(), (W as f32 * scale) as u32, (H as f32 * scale) as u32, scale).on_look(|s| Look { material: s.mat, ..Look::default() });
    u.pointer_leave();
    u.tick(5.0);
    u
}

#[test]
fn a_cached_subtree_that_scrolls_inks_only_the_band_and_what_changed_and_draws_the_frame_drawn_whole() {
    for scale in [1.0f32, 2.0] {
        let (w, h) = ((W as f32 * scale) as u32, (H as f32 * scale) as u32);
        let (Ok(mut part), Ok(mut whole)) = (Offscreen::try_new(w, h), Offscreen::try_new(w, h)) else { eprintln!("no GPU adapter here: skipped"); return };
        whole.r.partial = false;
        for mat in [Material::Plain, Material::Halftonr] {
            let (mut u, mut v) = (open(mat, scale), open(mat, scale));
            let look = u.look();
            let _ = part.render(&mut u, &look);
            let _ = whole.render(&mut v, &look);
            let (x, y) = (W as f32 * 0.4, H as f32 * 0.6);
            let mut t = 5.0;
            let mut moved = 0;
            // Down by 6 px a frame, then back up; a line in the sheet changes on the way.
            for f in 0..24 {
                t += 1.0 / 60.0;
                let dy = if f < 12 { 6.0 } else { -6.0 };
                for ui in [&mut u, &mut v] {
                    if f == 7 || f == 16 { ui.state.n += 1; ui.relayout(); }
                    ui.wheel(x, y, dy);
                    ui.tick(t);
                }
                let drawn = u.scene().plates.iter().map(|p| p.end - p.start).sum::<u32>();
                let a = part.render(&mut u, &look);
                let b = whole.render(&mut v, &look);
                let diff = a.iter().zip(&b).filter(|(p, q)| p != q).count();
                assert_eq!(diff, 0, "{mat:?} at scale {scale}, frame {f}: {diff} bytes differ from the frame drawn whole");
                let (c, path) = (part.r.cost, u.perf().path);
                assert!(drawn > 100, "{mat:?} frame {f}: the sheet is drawn as it moves ({drawn} instances)");
                if !path.moved { continue; }
                moved += 1;
                // Stage 1 draws the band, the edges, the glass and the changed line: not the sheet.
                let share = c.stage1 as f64 / (w * h) as f64;
                assert!(share < 0.4, "{mat:?} at scale {scale}, frame {f}: stage 1 drew {:.0}% of the window: {}", share * 100.0, path.line());
            }
            assert!(moved >= 20, "{mat:?} at scale {scale}: only {moved} of 24 frames moved the last frame ({})", u.perf().path.line());
            // A renderer made now draws the page from nothing: the same.
            let mut fresh = Offscreen::new(w, h);
            let a = part.render(&mut u, &look);
            let b = fresh.render(&mut u, &look);
            let diff = a.iter().zip(&b).filter(|(p, q)| p != q).count();
            assert_eq!(diff, 0, "{mat:?} at scale {scale}: {diff} bytes differ from a new renderer's frame");
        }
    }
}
