//! A scroll area: a fixed height that clips what it holds and scrolls it with the wheel, with a thin
//! bar for where it is.
use super::LINE;
use crate::el::{El, Paint, col};

pub fn scroll_area<M>(key: &str, height: f32, body: El<M>) -> El<M> {
    col().key(key).scroll().bar().height(height).radius(4.0).border(LINE, Paint::FAINT).pad(12.0, 10.0).child(body)
}
