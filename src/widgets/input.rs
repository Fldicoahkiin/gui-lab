//! A single-line text field. The application keeps the text and the caret (a `TextEdit`) in its state;
//! the field sends the edited value for every key it uses: printable characters insert at the caret,
//! Backspace and Delete remove, Left / Right / Home / End move the caret. There is no selection, and
//! no input method (composition of CJK text): that needs a hidden native text field and is not done.
use super::{CONTROL_H, LABEL, LINE, RADIUS};
use crate::el::{El, Key, Paint, row, text};
use taffy::AlignItems;

/// Text and a caret, which is a character index (0 = before the first character).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextEdit { pub text: String, pub caret: usize }

impl TextEdit {
    pub fn new(s: &str) -> Self { Self { text: s.to_owned(), caret: s.chars().count() } }
    fn byte(&self, at: usize) -> usize { self.text.char_indices().nth(at).map_or(self.text.len(), |(b, _)| b) }
    /// The caret as a byte index into `text`.
    pub fn caret_byte(&self) -> usize { self.byte(self.caret) }

    /// What a key does to the text, if anything. `max` is the most characters it may hold.
    pub fn apply(&self, k: Key, max: usize) -> Option<TextEdit> {
        let n = self.text.chars().count();
        let at = self.caret.min(n);
        let mut e = self.clone();
        match k {
            Key::Char(c) if !c.is_control() && n < max => { e.text.insert(self.byte(at), c); e.caret = at + 1; }
            Key::Backspace if at > 0 => { let b = self.byte(at - 1); e.text.replace_range(b..self.byte(at), ""); e.caret = at - 1; }
            Key::Delete if at < n => { e.text.replace_range(self.byte(at)..self.byte(at + 1), ""); }
            Key::Left if at > 0 => e.caret = at - 1,
            Key::Right if at < n => e.caret = at + 1,
            Key::Home if at > 0 => e.caret = 0,
            Key::End if at < n => e.caret = n,
            _ => return None,
        }
        Some(e)
    }
}

/// `width` is the field's width and `max` the most characters it takes; `placeholder` is shown, dim,
/// while it is empty.
pub fn input<M: Clone + 'static>(key: &str, edit: &TextEdit, placeholder: &str, width: f32, max: usize, msg: impl Fn(TextEdit) -> M + 'static) -> El<M> {
    let now = edit.clone();
    let field = if edit.text.is_empty() {
        text(placeholder, LABEL, Paint::DIM).caret(0)
    } else {
        text(edit.text.clone(), LABEL, Paint::INK).caret(edit.caret_byte())
    };
    row().key(key).width(width).height(CONTROL_H).pad(10.0, 0.0).radius(RADIUS).align(AlignItems::CENTER)
        .fill(Paint::GROUND).border(LINE, Paint::DIM)
        .on_key(move |k, _| now.apply(k, max).map(&msg))
        .child(field.key(format!("{key}/text")))
}
