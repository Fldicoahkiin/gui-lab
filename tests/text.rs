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
