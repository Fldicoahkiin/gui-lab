//! Scrolling moves the last frame (DESIGN.md §1.19): a slow scroll step shades only the band scrolled
//! into view, the edges the material reads across and what changed, counted by the renderer
//! (`Renderer::cost`), and the frame it draws is the frame drawn whole, byte for byte, at every offset,
//! down and back up, in several materials and at two scales.
use amenui::el::{El, Paint, col, row, space, text};
use amenui::offscreen::Offscreen;
use amenui::style::{Look, Material};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
const W: u32 = 480;
const H: u32 = 360;

struct S { mat: Material }
fn view(_: &S) -> El<()> {
    let rows = (0..60).fold(col().key("rows").gap(6.0).pad(12.0, 12.0), |c, i| {
        c.child(row().key(format!("row{i}")).gap(10.0).child(space(14.0, 14.0).radius(3.0).fill(if i % 3 == 0 { Paint::ACCENT } else { Paint::DIM }))
            .child(text(format!("Row {i}: a line of text to read as it scrolls past"), 14.0, Paint::TEXT)))
    });
    col().key("page").fill_width().fill_height()
        .child(text("A bar over the list that stays", 16.0, Paint::INK).key("bar"))
        .child(col().key("list").fill_width().grow(1.0).scroll().child(rows))
}
fn open(mat: Material, scale: f32) -> Ui<S, ()> {
    let mut u = Ui::new(S { mat }, view, |_, _| {}, fonts(), (W as f32 * scale) as u32, (H as f32 * scale) as u32, scale).on_look(|s| Look { material: s.mat, ..Look::default() });
    u.pointer_leave();
    u.tick(5.0);
    u
}

#[test]
fn a_slow_scroll_shades_only_the_band_it_brings_in_and_draws_the_frame_drawn_whole() {
    for scale in [1.0f32, 2.0] {
        let (w, h) = ((W as f32 * scale) as u32, (H as f32 * scale) as u32);
        let (Ok(mut part), Ok(mut whole)) = (Offscreen::try_new(w, h), Offscreen::try_new(w, h)) else { eprintln!("no GPU adapter here: skipped"); return };
        whole.r.partial = false;
        for mat in [Material::Plain, Material::Halftonr, Material::Charr, Material::Dottr] {
            let (mut u, mut v) = (open(mat, scale), open(mat, scale));
            let look = u.look();
            let _ = part.render(&mut u, &look);
            let _ = whole.render(&mut v, &look);
            let (x, y) = (W as f32 * 0.5, H as f32 * 0.6);
            let mut t = 5.0;
            let mut moved = 0;
            // Down by 6 px a frame (360 px/s, under the trace's calm speed), then back up.
            for f in 0..24 {
                t += 1.0 / 60.0;
                let dy = if f < 12 { 6.0 } else { -6.0 };
                for ui in [&mut u, &mut v] { ui.wheel(x, y, dy); ui.tick(t); }
                let a = part.render(&mut u, &look);
                let b = whole.render(&mut v, &look);
                let diff = a.iter().zip(&b).filter(|(p, q)| p != q).count();
                assert_eq!(diff, 0, "{mat:?} at scale {scale}, frame {f}: {diff} bytes differ from the frame drawn whole");
                let c = part.r.cost;
                if c.moved == 0 { continue; }
                moved += 1;
                // The band is 6 px of the list's height. The strips along the viewport's edges that the
                // material reads across (up to 12 px a side for dottr), the bar's spill and the scroll bar
                // are a fixed cost, large in a window this small: under half of it here, about a sixth of
                // a 1440 px wide one.
                let share = c.material as f64 / (w * h) as f64;
                assert!(share < 0.5, "{mat:?} at scale {scale}, frame {f}: the material shaded {:.0}% of the window", share * 100.0);
                assert!(c.stage1 < c.material, "{mat:?} at scale {scale}, frame {f}: stage 1 drew {} px, the material {}", c.stage1, c.material);
            }
            assert!(moved >= 20, "{mat:?} at scale {scale}: only {moved} of 24 frames moved the last frame");
        }
    }
}

#[test]
fn a_fast_scroll_with_its_trace_is_drawn_whole_in_the_viewport() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut u = open(Material::Plain, 1.0);
    let look = u.look();
    let _ = o.render(&mut u, &look);
    let mut t = 5.0;
    // 120 px a frame: 7200 px/s, the trace at full strength.
    for _ in 0..3 { t += 1.0 / 60.0; u.wheel(240.0, 200.0, 120.0); u.tick(t); let _ = o.render(&mut u, &look); }
    assert!(u.field().scrollo.iter().any(|o| o[2] > 0.0), "the trace shows");
    assert_eq!(o.r.cost.moved, 0, "with the trace showing the frame is not the last one moved");
}
