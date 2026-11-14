//! A picture at its own aspect ratio, filling the width it is given, with an optional caption. `slot`
//! picks which of the eight `PicFx` sets it turns into marks with when it loosens.
use super::LABEL;
use crate::el::{El, Paint, Role, col, picture, text};

pub fn image<M>(key: &str, id: u32, ratio: f32, slot: u8, caption: Option<&str>) -> El<M> {
    let mut c = col().key(key).gap(8.0).child(picture(id, ratio).key(format!("{key}/picture")).pic_fx(slot).role(Role::Img).label(caption.unwrap_or("图片")));
    if let Some(t) = caption { c = c.child(text(t, LABEL - 1.0, Paint::DIM)); }
    c
}
