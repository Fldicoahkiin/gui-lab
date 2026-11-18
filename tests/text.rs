//! Text input (DESIGN.md §1.8.1), without a GPU: every editing operation of `TextEdit`, input-method
//! composition, CJK and grapheme boundaries, undo; and the runtime: the pointer places the caret by the
//! laid-out clusters, drags select, double-click selects a word, Shift+arrows, Cmd+A / copy / cut /
//! paste / undo, composition shown inline, a long line scrolling inside its field, a multi-line field
//! moving by lines, and read-only selection across lines (source code, the article).
use amenui::el::{Key, Mods, Phase, TextInput};
use amenui::widgets::TextEdit;
const SHIFT: Mods = Mods { shift: true, cmd: false, alt: false };
const CMD: Mods = Mods { shift: false, cmd: true, alt: false };
fn key(k: Key, mods: Mods) -> TextInput { TextInput::Key { key: k, mods } }
fn ins(s: &str) -> TextInput { TextInput::Insert(s.into()) }
fn run(e: &TextEdit, evs: &[TextInput]) -> TextEdit {
    let mut e = e.clone();
    for ev in evs { if let Some(n) = e.handle(ev, 1000, false) { e = n; } }
    e
}

// ── TextEdit ─────────────────────────────────────────────────────────────────────────────────────

#[test]
fn insert_delete_and_move_by_graphemes() {
    let e = run(&TextEdit::default(), &[ins("a"), ins("b"), ins("c")]);
    assert_eq!((e.text.as_str(), e.caret), ("abc", 3));
    // A letter with a combining accent is one character for the caret and for Backspace.
    let e = run(&TextEdit::new("xe\u{301}"), &[key(Key::Left, Mods::default())]);
    assert_eq!(e.caret, 1, "one step over e + accent");
    let e = run(&TextEdit::new("xe\u{301}"), &[key(Key::Backspace, Mods::default())]);
    assert_eq!(e.text, "x");
    let e = run(&TextEdit::new("abc"), &[key(Key::Home, Mods::default()), key(Key::Delete, Mods::default())]);
    assert_eq!((e.text.as_str(), e.caret), ("bc", 0));
    // At the ends nothing changes, so the key is not used.
    assert!(TextEdit::new("abc").handle(&key(Key::Right, Mods::default()), 99, false).is_none());
    assert!(TextEdit::default().handle(&key(Key::Backspace, Mods::default()), 99, false).is_none());
    // The most characters.
    let e = TextEdit::new("abcd");
    assert!(e.handle(&ins("e"), 4, false).is_none());
}

#[test]
fn words_including_cjk() {
    let e = run(&TextEdit::new("hello big world"), &[key(Key::Left, CMD)]);
    assert_eq!(e.caret, 10, "to the start of `world`");
    let e = run(&e, &[key(Key::Left, CMD), key(Key::Left, CMD)]);
    assert_eq!(e.caret, 0);
    let e = run(&e, &[key(Key::Right, Mods { alt: true, ..Mods::default() })]);
    assert_eq!(e.caret, 5, "Alt jumps words too");
    let e = run(&TextEdit::new("hello world"), &[key(Key::Backspace, CMD)]);
    assert_eq!(e.text, "hello ");
    // Each ideograph is a word of its own.
    let e = run(&TextEdit::new("中文输入"), &[key(Key::Left, CMD)]);
    assert_eq!(e.caret, 3);
    let e = run(&TextEdit::new("中文"), &[key(Key::Left, Mods::default()), ins("x")]);
    assert_eq!((e.text.as_str(), e.caret), ("中x文", 2));
}

#[test]
fn selection_by_shift_cmd_a_and_pointer() {
    let e = run(&TextEdit::new("abcdef"), &[key(Key::Left, SHIFT), key(Key::Left, SHIFT)]);
    assert_eq!((e.selection(), e.selected()), (Some((4, 6)), "ef"));
    // Left without Shift collapses to the selection's start.
    let c = run(&e, &[key(Key::Left, Mods::default())]);
    assert_eq!((c.caret, c.selection()), (4, None));
    // Typing replaces the selection; Backspace removes it.
    assert_eq!(run(&e, &[ins("X")]).text, "abcdX");
    assert_eq!(run(&e, &[key(Key::Backspace, Mods::default())]).text, "abcd");
    let all = run(&e, &[key(Key::Char('a'), CMD)]);
    assert_eq!(all.selected(), "abcdef");
    assert_eq!(run(&all, &[key(Key::End, SHIFT)]).selected(), "abcdef");
    // The pointer: place, drag, double-click a word, triple-click everything (bytes in the shown text).
    let e = TextEdit::new("one two three");
    let p = |at, phase, clicks| TextInput::Pointer { at, phase, clicks, shift: false };
    let placed = run(&e, &[p(4, Phase::Start, 1)]);
    assert_eq!((placed.caret, placed.selection()), (4, None));
    let dragged = run(&e, &[p(4, Phase::Start, 1), p(8, Phase::Move, 1)]);
    assert_eq!(dragged.selected(), "two ");
    assert_eq!(run(&e, &[p(5, Phase::Start, 2)]).selected(), "two");
    assert_eq!(run(&e, &[p(5, Phase::Start, 2), p(10, Phase::Move, 2)]).selected(), "two three");
    assert_eq!(run(&e, &[p(5, Phase::Start, 3)]).selected(), "one two three");
    let shift = run(&placed, &[TextInput::Pointer { at: 13, phase: Phase::Start, clicks: 1, shift: true }]);
    assert_eq!(shift.selected(), "two three");
}

#[test]
fn cut_paste_undo_redo() {
    let e = run(&TextEdit::new("abcdef"), &[key(Key::Left, SHIFT), key(Key::Left, SHIFT)]);
    let cut = run(&e, &[TextInput::Cut]);
    assert_eq!(cut.text, "abcd");
    let pasted = run(&cut, &[key(Key::Home, Mods::default()), TextInput::Paste("ef".into())]);
    assert_eq!((pasted.text.as_str(), pasted.caret), ("efabcd", 2));
    let back = run(&pasted, &[key(Key::Char('z'), CMD)]);
    assert_eq!(back.text, "abcd");
    let back = run(&back, &[key(Key::Char('z'), CMD)]);
    assert_eq!(back.text, "abcdef");
    let fwd = run(&back, &[key(Key::Char('z'), Mods { shift: true, cmd: true, alt: false }), key(Key::Char('y'), CMD)]);
    assert_eq!(fwd.text, "efabcd");
    // Typing is one undo step per run; at least 50 steps are kept.
    let typed = run(&TextEdit::default(), &[ins("a"), ins("b"), ins("c")]);
    assert_eq!(run(&typed, &[key(Key::Char('z'), CMD)]).text, "");
    let mut e = TextEdit::default();
    for i in 0..60 { e = run(&e, &[TextInput::Paste(format!("{i},"))]); }
    for _ in 0..55 { e = run(&e, &[key(Key::Char('z'), CMD)]); }
    assert_eq!(e.text, "0,1,2,3,4,", "55 steps undone");
}

#[test]
fn composition_sequences() {
    let mut e = TextEdit::new("我");
    // Pinyin "ni" is composed, shown at the caret and underlined, then committed as 你.
    e = run(&e, &[TextInput::Compose { text: "n".into(), cursor: None }, TextInput::Compose { text: "ni".into(), cursor: None }]);
    assert_eq!(e.text, "我", "composition is not text yet");
    let (shown, caret, _, mark) = e.display();
    assert_eq!((shown.as_str(), caret, mark), ("我ni", 5, Some((3, 5))));
    // Keys belong to the input method while composing.
    assert!(e.handle(&key(Key::Left, Mods::default()), 99, false).is_none());
    e = run(&e, &[ins("你")]);
    assert_eq!((e.text.as_str(), e.caret, e.preedit.clone()), ("我你", 2, None));
    // Cancelled: nothing changes.
    let c = run(&e, &[TextInput::Compose { text: "hao".into(), cursor: None }, TextInput::Compose { text: String::new(), cursor: None }]);
    assert_eq!((c.text.as_str(), c.caret, c.preedit.clone()), ("我你", 2, None));
    // A composition replaces the selection; the commit is one undo step.
    let sel = run(&TextEdit::new("abc"), &[key(Key::Left, SHIFT)]);
    let done = run(&sel, &[TextInput::Compose { text: "zh".into(), cursor: None }, ins("中")]);
    assert_eq!(done.text, "ab中");
    assert_eq!(run(&done, &[key(Key::Char('z'), CMD)]).text, "ab");
}

#[test]
fn multiline_keeps_line_breaks() {
    let e = TextEdit::new("ab");
    let e = e.handle(&key(Key::Enter, Mods::default()), 99, true).expect("Enter breaks the line");
    assert_eq!(e.text, "ab\n");
    assert!(TextEdit::new("ab").handle(&key(Key::Enter, Mods::default()), 99, false).is_none(), "not in a single line");
    assert_eq!(TextEdit::default().handle(&TextInput::Paste("a\nb".into()), 99, false).unwrap().text, "a b");
    assert_eq!(TextEdit::default().handle(&TextInput::Paste("a\r\nb".into()), 99, true).unwrap().text, "a\nb");
}

/// One line of text in one face, alone on a light page (`ink` tests).
mod inks {
    use amenui::el::{El, Font, Paint, col, text};
    pub struct S { pub face: u8, pub size: f32, pub s: &'static str }
    pub fn view(s: &S) -> El<()> {
        col().key("page").fill_width().fill_height().pad(20.0, 20.0)
            .child(text(s.s, s.size, Paint::INK).font(Font::Body).fonts([s.face, s.face, s.face]).key("t"))
    }
    pub fn look(_: &S) -> amenui::style::Look { amenui::style::Look { material: amenui::style::Material::Plain, theme: amenui::style::LIGHT, ..amenui::style::Look::default() } }
}

#[test]
fn line_inks_match_the_drawn_ink_and_the_faces_numbers() {
    use amenui::ui::Ui;
    let fonts = || amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts");
    let (w, h) = (900u32, 160u32);
    let gpu = amenui::offscreen::Offscreen::try_new(w, h).ok();
    let mut gpu = gpu;
    let strings = ["sense", "Tj dolly", "HEXAGON", "Mixed caps", "quick, brown", "WAVE 42"];
    for face in [0u8, 1, 2, 3, 8] { for size in [11.0f32, 14.0, 20.0, 48.0] { for s in strings {
        let mut ui = Ui::new(inks::S { face, size, s }, inks::view, |_, _| {}, fonts(), w, h, 1.0).on_look(inks::look);
        ui.tick(1.0);
        let shaped = ui.counters().shaped;
        let li = ui.line_inks("t");
        assert_eq!(li.len(), 1, "face {face} {size} {s:?}: one line");
        let l = li[0];
        assert!(l.cap_top < l.baseline && l.x_top < l.baseline && l.cap_top <= l.x_top + 0.01, "face {face} {size} {s:?}: {l:?}");
        // Caps sit on the baseline and reach the cap height.
        if s.chars().all(|c| c.is_ascii_uppercase() || c == ' ' || c.is_ascii_digit()) && face != 1 && face != 2 {
            assert!((l.ink.bottom() - l.baseline).abs() <= 1.0, "face {face} {size} {s:?}: caps end at {} on a baseline at {}", l.ink.bottom(), l.baseline);
            assert!((l.ink.y - l.cap_top).abs() <= 1.0 + 0.02 * size, "face {face} {size} {s:?}: caps start at {} under a cap top at {}", l.ink.y, l.cap_top);
        }
        // Moving the pointer shapes nothing.
        for k in 0..60 { ui.pointer_move(30.0 + k as f32, 40.0); }
        let _ = ui.line_inks("t");
        assert_eq!(ui.counters().shaped, shaped, "face {face} {size} {s:?}: the pointer shaped text");
        let Some(o) = gpu.as_mut() else { continue };
        let look = ui.look();
        let px = o.render(&mut ui, &look);
        let ground = [px[0] as i32, px[1] as i32, px[2] as i32];
        let lum = |p: &[u8]| (p[0] as i32 - ground[0]).abs() + (p[1] as i32 - ground[1]).abs() + (p[2] as i32 - ground[2]).abs();
        let full = 3 * 200;
        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
        for y in 0..h { for x in 0..w { let i = ((y * w + x) * 4) as usize; if lum(&px[i..i + 3]) * 2 > full { x0 = x0.min(x); y0 = y0.min(y); x1 = x1.max(x + 1); y1 = y1.max(y + 1); } } }
        let r = l.ink;
        for (a, b, edge) in [(r.x, x0 as f32, "left"), (r.y, y0 as f32, "top"), (r.right(), x1 as f32, "right"), (r.bottom(), y1 as f32, "bottom")] {
            assert!((a - b).abs() <= 1.0, "face {face} {size} {s:?}: the ink's {edge} is {a}, drawn {b}");
        }
    } } }
}

/// One paragraph clamped to `lines` lines in a column `w` px wide.
mod clamped {
    use amenui::el::{El, Font, Paint, col, text};
    pub struct S { pub s: String, pub face: u8, pub w: f32, pub lines: u16 }
    pub fn view(s: &S) -> El<()> {
        col().key("page").fill_width().fill_height().pad(10.0, 10.0)
            .child(col().key("box").width(s.w).child(text(s.s.clone(), 14.0, Paint::TEXT).font(Font::Body).fonts([s.face, s.face, s.face]).clamp(s.lines).key("t")))
    }
}

#[test]
fn clamped_text_ends_in_an_ellipsis_within_its_lines_and_grows_with_its_width() {
    use amenui::ui::Ui;
    let fonts = || amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts");
    let words = ["the", "framework", "draws", "every", "glyph", "with", "a", "true", "outline,", "loosens", "it:", "and", "brings", "it", "back", "together", "again", "-", "(quietly)", "extraordinarily"];
    let paras: Vec<String> = (0..40).map(|i| (0..(6 + i * 3)).map(|j| words[(i * 7 + j * 3) % words.len()]).collect::<Vec<_>>().join(" ")).collect();
    let forbidden = [' ', ',', '.', ':', ';', '\u{2014}', '\u{2013}', '-', '('];
    let mut ui = Ui::new(clamped::S { s: String::new(), face: 0, w: 200.0, lines: 2 }, clamped::view, |_, _| {}, fonts(), 800, 400, 1.0);
    for (pi, para) in paras.iter().enumerate() { for face in [0u8, 2, 3, 8] { for lines in [1u16, 2, 3, 5] {
        let step = if pi < 3 { 1 } else { 37 };
        let (mut last_shown, mut fitted) = (0usize, false);
        for w in (120..=600).step_by(step) {
            ui.state = clamped::S { s: para.clone(), face, w: w as f32, lines };
            ui.relayout();
            let tag = format!("para {pi} face {face} {lines} lines at {w}");
            let boxes = ui.line_boxes("t");
            assert!(boxes.len() <= lines as usize, "{tag}: {} lines", boxes.len());
            let inner = ui.node("t").unwrap().inner;
            let inks = ui.line_inks("t");
            if let Some(l) = inks.last() { assert!(l.ink.right() <= inner.right() + 1.0, "{tag}: the last line's ink ends at {}, the box at {}", l.ink.right(), inner.right()); }
            let shown = ui.instances_of("t").len();
            let cut = ui.clamped("t");
            assert_eq!(ui.export_tree().find("t").unwrap().text.as_deref(), Some(para.as_str()), "{tag}: assistive technology reads all of it");
            if !cut {
                // It fits: the same as without the clamp.
                let mut free = Ui::new(clamped::S { s: para.clone(), face, w: w as f32, lines: 0 }, clamped::view, |_, _| {}, fonts(), 800, 400, 1.0);
                free.tick(0.0);
                assert_eq!(free.line_boxes("t"), boxes, "{tag}: fits, yet differs from no clamp");
            } else {
                let full = amenui::text::Text::new(fonts()).clamp_lines(amenui::text::Para { s: para, face, size: 14.0, line: 22.0, width: inner.w + 1.0, print: 0.0, around: None, track: 0.0 }, lines).expect("cut");
                let before = full.trim_end_matches(['\u{2026}', '.']).chars().last();
                assert!(before.is_none_or(|c| !forbidden.contains(&c)), "{tag}: {full:?} has {before:?} before the ellipsis");
            }
            // Wider, it never shows less: more of it, or all of it from the width where it fits on.
            assert!(!(fitted && cut), "{tag}: cut again after it fitted at a narrower width");
            if cut { assert!(shown >= last_shown, "{tag}: {shown} glyphs after {last_shown}"); last_shown = shown; } else { fitted = true; }
        }
    } } }
}
