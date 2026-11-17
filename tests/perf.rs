//! The frame-time readout and automatic quality (DESIGN.md §1.12), without a GPU.
use amenui::perf::{AutoQuality, BAD_RUN, GOOD_RUN, Perf};

#[test]
fn auto_quality_steps_down_and_back_up() {
    let mut a = AutoQuality::default();
    assert_eq!(a.quality(), 1.0);
    // Slow frames: one level down after a run of them, then the next.
    for _ in 0..BAD_RUN + 5 { a.feed(20.0); }
    assert_eq!(a.quality(), 0.75);
    for _ in 0..BAD_RUN { a.feed(20.0); }
    assert_eq!(a.quality(), 0.5);
    for _ in 0..BAD_RUN * 3 { a.feed(20.0); }
    assert_eq!(a.quality(), 0.5, "never below the last level");
    // One slow frame among fast ones does not count; a long run of fast ones restores a level.
    for i in 0..GOOD_RUN + 10 { a.feed(if i == 50 { 30.0 } else { 4.0 }); }
    assert_eq!(a.quality(), 0.75);
    for _ in 0..GOOD_RUN + 10 { a.feed(4.0); }
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
