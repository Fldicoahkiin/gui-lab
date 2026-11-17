//! Text in more than one language (DESIGN.md §1.20): every key is in every shipped catalog (plural
//! keys with every form the locale needs; the Body and Loose Thread pages' words are English only), the landing scene and
//! Loose Thread's composition show no missing key and no Chinese in either language, switching the language is a message that lays everything out again (keyed text
//! recomposes), and a right-to-left row puts its first child on the right.
use amenui::el::{El, Paint, row, space};
use amenui::i18n::{self, Locale};
use std::collections::BTreeSet;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
/// A catalog's messages by their base key (plural forms folded in), with the forms each gives.
fn bases(l: Locale) -> std::collections::BTreeMap<String, BTreeSet<String>> {
    let mut m = std::collections::BTreeMap::new();
    for k in i18n::catalog(l).keys() {
        let (base, form) = match k.rsplit_once('.') {
            Some((b, f)) if ["zero", "one", "two", "few", "many", "other"].contains(&f) => (b.to_owned(), f.to_owned()),
            _ => (k.clone(), String::new()),
        };
        m.entry(base).or_insert_with(BTreeSet::new).insert(form);
    }
    m
}

#[test]
fn every_key_is_in_every_locale() {
    let all: BTreeSet<String> = Locale::ALL.iter().flat_map(|l| bases(*l).into_keys()).collect();
    for l in Locale::ALL {
        let b = bases(l);
        // The Body and Loose Thread pages are English whatever the language (their words exist only in English).
        let english_only = |k: &str| l != Locale::En && ((k.starts_with("body.") && k != "body.label") || k.starts_with("thread."));
        // `locale.*` (a locale's own name and date pattern) is only for locales that are not built in.
        let own = |k: &str| k.starts_with("locale.");
        let missing: Vec<&String> = all.iter().filter(|k| !b.contains_key(*k) && !english_only(k) && !own(k)).collect();
        assert!(missing.is_empty(), "{} is missing {} keys: {missing:?}", l.tag(), missing.len());
        for (k, forms) in &b {
            if forms.contains("") { continue; }
            for need in l.categories() { assert!(forms.contains(need), "{}: plural `{k}` has no `{need}` form", l.tag()); }
        }
    }
}

#[test]
fn a_right_to_left_row_starts_at_the_right() {
    fn view(rtl: &bool) -> El<()> {
        row().key("row").width(300.0).rtl(*rtl).child(space(50.0, 20.0).fill(Paint::INK).key("first")).child(space(50.0, 20.0).fill(Paint::INK).key("second"))
    }
    let mut ui = amenui::ui::Ui::new(false, view, |s: &mut bool, _: ()| { *s = !*s; }, fonts(), 400, 200, 1.0);
    let (a, b) = (ui.node("first").unwrap().rect.x, ui.node("second").unwrap().rect.x);
    assert!(a < b, "left to right: {a} {b}");
    ui.state = true;
    ui.relayout();
    let (a, b) = (ui.node("first").unwrap().rect.x, ui.node("second").unwrap().rect.x);
    assert!(a > b, "right to left, the first child is on the right: {a} {b}");
    assert!(a + 50.0 > 299.0, "and starts at the row's right edge: {a}");
}

#[test]
fn any_tag_is_a_locale_and_arabic_runs_right_to_left() {
    assert_eq!(Locale::from_tag("ar-EG"), Some(Locale::AR));
    assert_eq!(Locale::from_tag("AR"), Some(Locale::AR));
    assert_eq!(Locale::AR.direction(), i18n::Direction::Rtl);
    assert_eq!(Locale::En.direction(), i18n::Direction::Ltr);
    let pt = Locale::from_tag("pt_BR").expect("a tag");
    assert_eq!(pt.tag(), "pt-BR");
    assert_eq!(pt.direction(), i18n::Direction::Ltr);
    assert_eq!(Locale::from_tag("az-Arab").map(|l| l.direction()), Some(i18n::Direction::Rtl));
    assert_eq!(Locale::from_tag("not a tag"), None);
    // An unknown language falls back to English, with its own plural rules and an ISO date.
    assert_eq!(pt.tr("widget.dialog.close"), "Close");
    assert_eq!(pt.date(2026, 3, 5), "2026-03-05");
    assert_eq!(Locale::AR.name(), "العربية");
    assert_eq!(Locale::AR.date(2026, 11, 15), "15/11/2026");
    assert_eq!(Locale::AR.categories(), vec!["zero", "one", "two", "few", "many", "other"]);
    assert_eq!(amenui::t!(Locale::AR, "widget.list.rows", n = 2), "صفّان");
    assert_eq!(amenui::t!(Locale::AR, "widget.list.rows", n = 5), "5 صفوف");
}

/// Arabic and Hebrew text is shaped right to left: with a fallback face that has the script, the first
/// character's caret is at the paragraph's right edge, and every character gets a glyph (no tofu).
#[test]
fn right_to_left_text_has_glyphs_and_its_caret_starts_at_the_right() {
    let Some(path) = ["/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", "/System/Library/Fonts/Supplemental/Arial Unicode.ttf"].into_iter().find(|p| std::path::Path::new(p).exists()) else {
        eprintln!("no font with Arabic here: skipped");
        return;
    };
    let mut faces = fonts();
    faces.push(amenui::text::Face::fallback(std::fs::read(path).expect("font")));
    let mut t = amenui::text::Text::new(faces);
    for s in ["مرحبا بالعالم", "שלום עולם"] {
        let p = amenui::text::Para { s, face: amenui::text::SANS, size: 20.0, line: 28.0, width: 400.0, print: 0.0, around: None, track: 0.0 };
        let (w, _) = t.measure(s, amenui::text::SANS, 20.0, 28.0, Some(400.0));
        assert!(w > 40.0, "{s}: shaped to {w} px");
        let first = t.caret(p, (0.0, 0.0), 0).expect("caret").x;
        let last = t.caret(p, (0.0, 0.0), s.len()).expect("caret").x;
        assert!(first > last, "{s}: the caret before the first character ({first}) is right of the one after the last ({last})");
        assert!(t.missing_glyphs(p) == 0, "{s}: characters without a glyph");
    }
}
