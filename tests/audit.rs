//! The audit (`amenui::audit`): five pages broken on purpose (text past the window, grey on grey, two
//! lines over each other, 6 px text, something that never stops moving) each report their finding at
//! the right level; every recipe in every material, dark and light, has no P0 or P1. Needs a GPU
//! adapter (lavapipe will do); without one the tests say so and pass.
use amenui::audit::{Finding, Kind, Level};
use amenui::drive::{Driven, RenderSpec, audit, audit_with};
use amenui::offscreen::Offscreen;
use amenui::el::{El, Paint, col, layers, space, text};
use amenui::error::LookError;
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

const W: u32 = 480;
const H: u32 = 240;

fn outside(_: &()) -> El<()> {
    col().pad(20.0, 20.0).child(text("this line runs on well past the right edge of the window and is cut off there", 22.0, Paint::TEXT).nowrap().key("long"))
}
fn grey(_: &()) -> El<()> { col().pad(20.0, 20.0).child(text("grey text on a grey ground", 18.0, Paint::tone(0.14)).key("grey")) }
fn stacked(_: &()) -> El<()> {
    col().pad(20.0, 20.0).child(layers([
        text("two lines drawn over each other", 18.0, Paint::TEXT).key("under"),
        text("two lines drawn over each other", 18.0, Paint::TEXT).key("over").shift(2.0, 3.0),
    ]))
}
fn tiny(_: &()) -> El<()> { col().pad(20.0, 20.0).child(text("six pixels tall", 6.0, Paint::TEXT).key("tiny")) }
fn moving(t: &f32) -> El<()> { col().pad(20.0, 20.0).child(space(60.0 + (t * 3.0).sin() * 30.0, 12.0).fill(Paint::INK).key("bar")) }
fn moving_on_purpose(t: &f32) -> El<()> { col().pad(20.0, 20.0).child(space(60.0 + (t * 3.0).sin() * 30.0, 12.0).fill(Paint::INK).key("bar").ongoing()) }
fn boxed(_: &()) -> El<()> {
    col().pad(20.0, 20.0).child(col().key("box").width(200.0).clip().child(text("this line is longer than the box that clips it", 18.0, Paint::TEXT).nowrap().key("boxed")))
}
fn calm(_: &()) -> El<()> { col().pad(20.0, 20.0).gap(8.0).child(text("A calm page", 22.0, Paint::INK).key("title")).child(text("Everything on it reads.", 15.0, Paint::DIM).key("note")) }

fn broken(page: &str, faces: Vec<amenui::text::Face>, (w, h, s): (u32, u32, f32), _: (Option<amenui::style::Material>, Option<usize>, Option<amenui::i18n::Locale>)) -> Result<Box<dyn Driven>, LookError> {
    let still = |v: fn(&()) -> El<()>| -> Box<dyn Driven> { Box::new(Ui::new((), v, |_, _| {}, fonts(), w, h, s)) };
    let tick = |s: &mut f32, t: f32| { *s = t; true };
    Ok(match page {
        "outside" => still(outside),
        "grey" => still(grey),
        "stacked" => still(stacked),
        "tiny" => still(tiny),
        "calm" => still(calm),
        "boxed" => still(boxed),
        "moving" => Box::new(Ui::new(0.0f32, moving, |_, _| {}, faces, w, h, s).on_tick(tick)),
        "moving-on-purpose" => Box::new(Ui::new(0.0f32, moving_on_purpose, |_, _| {}, faces, w, h, s).on_tick(tick)),
        _ => return Err(LookError::UnknownPage { given: page.into(), recipes: String::new() }),
    })
}

fn findings(page: &str, o: &mut Offscreen) -> Vec<Finding> {
    let spec = RenderSpec { page: page.into(), w: W, h: H, scale: 1.0, material: None, theme: Some("light".into()), at_ms: 600.0, events: vec![], locale: None };
    audit_with(&spec, fonts(), Some(o), &broken).expect("audit").1
}

fn has(f: &[Finding], kind: Kind, level: Level, key: &str) -> bool { f.iter().any(|f| f.kind == kind && f.level == level && f.key == key) }

#[test]
fn five_broken_pages_each_report_what_is_wrong() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let calm = findings("calm", &mut o);
    assert!(calm.is_empty(), "a calm page has nothing: {calm:?}");
    let f = findings("outside", &mut o);
    assert!(has(&f, Kind::Clipped, Level::P0, "long"), "text past the window: {f:?}");
    // What to do says which side, how far and what cuts it.
    let cut = f.iter().find(|f| f.kind == Kind::Clipped && f.key == "long").expect("clipped");
    assert!(cut.fix.contains("right") && cut.fix.contains("the window") && cut.fix.chars().any(|c| c.is_ascii_digit()), "{}", cut.fix);
    let f = findings("boxed", &mut o);
    let cut = f.iter().find(|f| f.kind == Kind::Clipped && f.key == "boxed" && f.level == Level::P0).unwrap_or_else(|| panic!("clipped by its box: {f:?}"));
    assert!(cut.fix.contains("right") && cut.fix.contains("`box`") && cut.fix.contains(" px"), "{}", cut.fix);
    let f = findings("grey", &mut o);
    assert!(has(&f, Kind::Contrast, Level::P0, "grey"), "grey on grey: {f:?}");
    let f = findings("stacked", &mut o);
    assert!(f.iter().any(|f| f.kind == Kind::Overlap && f.level == Level::P1 && (f.key == "under" || f.key == "over")), "two lines over each other: {f:?}");
    let f = findings("tiny", &mut o);
    assert!(has(&f, Kind::Small, Level::P1, "tiny"), "6 px text: {f:?}");
    let f = findings("moving", &mut o);
    assert!(f.iter().any(|f| f.kind == Kind::Restless && f.level == Level::P1), "a page that never settles: {f:?}");
    // The same motion declared as meant (`El::ongoing`) is not a finding.
    let f = findings("moving-on-purpose", &mut o);
    assert!(!f.iter().any(|f| f.kind == Kind::Restless), "declared motion: {f:?}");
    // Every finding says what to do.
    for page in ["outside", "grey", "stacked", "tiny", "moving"] {
        for f in findings(page, &mut o) { assert!(!f.fix.is_empty(), "{page}: {f:?}"); }
    }
}

#[test]
fn every_recipe_in_every_material_dark_and_light_has_no_p0_or_p1() {
    let Ok(mut o) = Offscreen::try_new(800, 600) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut bad = vec![];
    for page in amenui::recipes::NAMES {
        for (_, material) in amenui::style::MATERIALS {
            for theme in ["dark", "light"] {
                let spec = RenderSpec { page: format!("recipe-{page}"), w: 800, h: 600, scale: 1.0, material: Some(material.into()), theme: Some(theme.into()), at_ms: 1500.0, events: vec![], locale: None };
                let (_, f) = audit(&spec, fonts(), Some(&mut o)).expect("audit");
                bad.extend(f.into_iter().filter(|f| matches!(f.level, Level::P0 | Level::P1)).map(|f| format!("{page} {material} {theme}: {f}")));
            }
        }
    }
    assert!(bad.is_empty(), "{} findings:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn text_a_material_wears_away_at_rest_is_found() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut ui = Ui::new((), calm, |_, _| {}, fonts(), W, H, 1.0);
    ui.tick(1.0);
    let look = amenui::style::Look { material: amenui::style::Material::Plain, ..ui.look() };
    o.r.partial = false;
    let px = o.render(&mut ui, &look);
    o.r.hide_glyphs = true;
    ui.invalidate();
    let ground = o.render(&mut ui, &look);
    o.r.hide_glyphs = false;
    // The same frame is the same ink: nothing.
    assert!(amenui::audit::eroded(&ui, (&px, &ground), (&px, &ground), "plain").is_empty());
    // A material that drew none of the strokes (its frame is the ground) wears both texts away.
    let f = amenui::audit::eroded(&ui, (&ground, &ground), (&px, &ground), "worn");
    for key in ["title", "note"] { assert!(has(&f, Kind::Eroded, Level::P1, key), "{key}: {f:?}"); }
    // Half of every other row of pixels taken away is still under 85 %.
    let mut half = px.clone();
    for (k, row) in half.chunks_mut((W * 4) as usize).enumerate() { if k % 2 == 0 { let g = &ground[k * (W * 4) as usize..(k + 1) * (W * 4) as usize]; row.copy_from_slice(g); } }
    let f = amenui::audit::eroded(&ui, (&half, &ground), (&px, &ground), "half");
    assert!(has(&f, Kind::Eroded, Level::P1, "title"), "{f:?}");
}
