//! The frame-time readout and automatic quality (DESIGN.md §1.12), without a GPU.
use amenui::perf::{AutoQuality, Perf, CALM_MS, MOVING_FRAMES};

/// Feeds `n` frames `every` ms apart from `t`, each costing `cost` ms; the time after the last.
fn run(a: &mut AutoQuality, t: f64, n: usize, every: f64, cost: f32, moving: bool) -> f64 {
    let mut t = t;
    for _ in 0..n { t += every; a.feed(t, cost, moving); }
    t
}

#[test]
fn auto_quality_goes_by_the_intervals_while_something_moves() {
    let mut a = AutoQuality::default();
    assert_eq!(a.quality(), 1.0);
    // A 60 Hz display at rest: the display frame is measured, nothing steps.
    let mut t = run(&mut a, 0.0, 120, 16.7, 3.0, false);
    assert!((a.frame_ms - 16.7).abs() < 0.1, "{}", a.frame_ms);
    // Slow frames while nothing moves never step it.
    t = run(&mut a, t, 60, 40.0, 3.0, false);
    assert_eq!(a.quality(), 1.0);
    t = run(&mut a, t, 60, 16.7, 3.0, false);
    // Scrolling at 30 fps (cheap on the CPU: the GPU is what is slow): one level down after the run.
    t = run(&mut a, t, MOVING_FRAMES - 1, 33.4, 3.0, true);
    assert_eq!(a.quality(), 1.0, "not before {MOVING_FRAMES} frames");
    t = run(&mut a, t, 1, 33.4, 3.0, true);
    assert_eq!(a.quality(), 0.75);
    t = run(&mut a, t, MOVING_FRAMES, 33.4, 3.0, true);
    assert_eq!(a.quality(), 0.5);
    t = run(&mut a, t, MOVING_FRAMES * 3, 33.4, 3.0, true);
    assert_eq!(a.quality(), 0.5, "never below the last level");
    // In time again: a missed frame starts the wait over; 2 s of moving frames in time and cheap
    // restore a level.
    let n = (CALM_MS / 16.7) as usize;
    t = run(&mut a, t, n - 10, 16.7, 3.0, true);
    t = run(&mut a, t, 1, 40.0, 3.0, true);
    t = run(&mut a, t, n - 10, 16.7, 3.0, true);
    assert_eq!(a.quality(), 0.5, "the wait started over");
    t = run(&mut a, t, 20, 16.7, 3.0, true);
    assert_eq!(a.quality(), 0.75);
    // Frames in time but costly (over 60 % of the display frame) do not.
    t = run(&mut a, t, n * 2, 16.7, 12.0, true);
    assert_eq!(a.quality(), 0.75);
    // Moving frames are what count: time at rest does not.
    t = run(&mut a, t, n * 3, 16.7, 3.0, false);
    assert_eq!(a.quality(), 0.75);
    run(&mut a, t, n + 2, 16.7, 3.0, true);
    assert_eq!(a.quality(), 1.0);
}

#[test]
fn summary_of_the_last_second() {
    let mut p = Perf::default();
    for k in 0..5 { p.pending.update_layout = k as f32; p.pending.scene = 1.0; p.frame(2.0); }
    let s = p.summary(amenui::perf::now_ms());
    assert_eq!(s.fps, 5);
    assert_eq!(s.update_layout, [4.0, 2.0, 4.0]);
    assert_eq!(s.submit, [2.0, 2.0, 2.0]);
    assert_eq!(s.total[2], 7.0);
    assert_eq!(p.drawn, 5);
    // Nothing in the last second, much later.
    assert_eq!(p.summary(amenui::perf::now_ms() + 5000.0).fps, 0);
}
