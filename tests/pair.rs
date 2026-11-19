//! Distances between glyph outlines (`pair`, DESIGN.md §1.6.6), checked against the coverage the text
//! system rasters: set `target` px apart at their closest by `pair::spaced`, the letters' ink pixels are
//! that far apart to within a pixel, and set by `pair::spaced_area` they hold that white between them.
use amenui::el::Font;
use amenui::pair::{area, gap, spaced, spaced_area};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

#[derive(Default)]
struct S { faces: Option<amenui::pair::Faces> }

/// The ink pixels (coverage at least half) of `ch` at `px`, from the pen at `x0`, as pixel centres
/// (x right, y down from the baseline).
fn ink(u: &mut Ui<S, ()>, ch: char, px: f32, x0: f32) -> Vec<(f32, f32)> {
    let r = u.text_mut().raster(0, ch, px).expect("raster");
    let mut v = vec![];
    for y in 0..r.h { for x in 0..r.w {
        if r.data[(y * r.w + x) as usize] >= 128 { v.push((x0 + r.left as f32 + x as f32 + 0.5, -r.top as f32 + y as f32 + 0.5)); }
    } }
    v
}

#[test]
fn a_pair_set_by_its_gap_is_that_far_apart_in_its_coverage() {
    let mut u: Ui<S, ()> = Ui::new(S::default(), |_| amenui::el::col(), |_, _| {}, fonts(), 400, 300, 1.0).on_faces(|s, f| s.faces = Some(f));
    let faces = u.state.faces.clone().expect("the faces were handed over");
    let px = 176.0;
    let word: Vec<char> = "Voyage".chars().collect();
    for target in [0.0f32, 8.0, 24.0] {
        for w in word.windows(2) {
            let (a, b) = (faces.glyph(Font::Face(0), w[0], px).expect("a"), faces.glyph(Font::Face(0), w[1], px).expect("b"));
            let dx = spaced(&a, &b, target);
            assert!((gap(&a, &b, dx) - target).abs() < 0.05, "{}{}: the gap at its place is {}", w[0], w[1], gap(&a, &b, dx));
            // The coverage: the nearest ink pixels of the two, centre to centre.
            let (ia, ib) = (ink(&mut u, w[0], px, 0.0), ink(&mut u, w[1], px, dx));
            let near = ia.iter().flat_map(|p| ib.iter().map(move |q| (p.0 - q.0).hypot(p.1 - q.1))).fold(f32::INFINITY, f32::min);
            // Pixel centres are up to a pixel further than the edges, and touching pixels are 1 apart.
            assert!(near >= target - 1.0 && near <= target + 1.5, "{}{} at {target}: the ink is {near} apart", w[0], w[1]);
        }
    }
    // By the white between them.
    let (a, b) = (faces.glyph(Font::Face(0), 'o', px).unwrap(), faces.glyph(Font::Face(0), 'y', px).unwrap());
    let dx = spaced_area(&a, &b, 1240.0);
    assert!((area(&a, &b, dx) - 1240.0).abs() < 40.0, "{}", area(&a, &b, dx));
    assert!(gap(&a, &b, 0.0) < 0.0, "drawn over each other they overlap");
    // Nine pairs measured in a frame's time.
    let t0 = std::time::Instant::now();
    let mut sum = 0.0;
    for k in 0..9 { sum += gap(&a, &b, dx + k as f32); }
    eprintln!("nine pair distances: {:.3} ms (debug build) {sum}", t0.elapsed().as_secs_f64() * 1000.0);
}
