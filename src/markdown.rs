//! A small Markdown subset rendered as elements (DESIGN.md §5): the documentation's prose lives in
//! `docs/site/<locale>/*.md` and is drawn by the framework itself. What it reads: `#`, `##`, `###`
//! headings; paragraphs (consecutive lines); `-` and `1.` lists; fenced code blocks (```); tables
//! (`| a | b |` with a `|---|` rule); `[text](target)` links; `` `inline code` ``; `**strong**` is
//! shown plain. Inline code and links are drawn in the paragraph's own type (one run of text per
//! paragraph), the links' targets listed under it as buttons, so line breaking stays the type
//! system's. Pure: no GPU, no state.
use crate::el::{El, Font, Paint, Role, col, row, space, text, text_lh};
use taffy::AlignItems;

/// A block of the document.
#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Heading(u8, String),
    Para(String),
    List(bool, Vec<String>),
    Code(String, String),
    Table(Vec<String>, Vec<Vec<String>>),
}
/// The blocks of a document.
pub fn parse(src: &str) -> Vec<Block> {
    let mut out = vec![];
    let lines: Vec<&str> = src.lines().collect();
    let mut i = 0;
    let mut para: Vec<&str> = vec![];
    let flush = |para: &mut Vec<&str>, out: &mut Vec<Block>| { if !para.is_empty() { out.push(Block::Para(para.join(" ").trim().to_owned())); para.clear(); } };
    while i < lines.len() {
        let l = lines[i];
        let t = l.trim_end();
        if let Some(lang) = t.trim_start().strip_prefix("```") {
            flush(&mut para, &mut out);
            let mut code = vec![];
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with("```") { code.push(lines[i]); i += 1; }
            out.push(Block::Code(lang.trim().to_owned(), code.join("\n")));
        } else if let Some(h) = heading(t) {
            flush(&mut para, &mut out);
            out.push(h);
        } else if t.trim_start().starts_with('|') {
            flush(&mut para, &mut out);
            let cells = |s: &str| s.trim().trim_matches('|').split('|').map(|c| c.trim().to_owned()).collect::<Vec<_>>();
            let head = cells(t);
            let mut rows = vec![];
            i += 1;
            while i < lines.len() && lines[i].trim_start().starts_with('|') {
                if !lines[i].contains("---") { rows.push(cells(lines[i])); }
                i += 1;
            }
            out.push(Block::Table(head, rows));
            continue;
        } else if item(t).is_some() {
            flush(&mut para, &mut out);
            let ordered = item(t).is_some_and(|(o, _)| o);
            let mut items = vec![];
            while i < lines.len() && let Some((_, s)) = item(lines[i].trim_end()) { items.push(s.to_owned()); i += 1; }
            out.push(Block::List(ordered, items));
            continue;
        } else if t.trim().is_empty() {
            flush(&mut para, &mut out);
        } else {
            para.push(t.trim());
        }
        i += 1;
    }
    flush(&mut para, &mut out);
    out
}
fn heading(t: &str) -> Option<Block> {
    let n = t.chars().take_while(|c| *c == '#').count();
    (1..=3).contains(&n).then(|| t[n..].strip_prefix(' ').map(|s| Block::Heading(n as u8, s.trim().to_owned()))).flatten()
}
fn item(t: &str) -> Option<(bool, &str)> {
    let s = t.trim_start();
    if let Some(r) = s.strip_prefix("- ") { return Some((false, r)); }
    let d = s.chars().take_while(|c| c.is_ascii_digit()).count();
    (d > 0).then(|| s[d..].strip_prefix(". ").map(|r| (true, r))).flatten()
}
/// A paragraph's text with its marks taken off, and its links (text, target).
pub fn inline(s: &str) -> (String, Vec<(String, String)>) {
    let mut out = String::new();
    let mut links = vec![];
    let mut rest = s;
    while let Some(i) = rest.find(['[', '`', '*']) {
        out.push_str(&rest[..i]);
        let r = &rest[i..];
        if let Some(t) = r.strip_prefix("**") { rest = t; continue; }
        if let Some(t) = r.strip_prefix('`') && let Some(j) = t.find('`') { out.push_str(&t[..j]); rest = &t[j + 1..]; continue; }
        if r.starts_with('[') && let Some(j) = r.find("](") && let Some(k) = r[j..].find(')') {
            let label = &r[1..j];
            out.push_str(label);
            links.push((label.to_owned(), r[j + 2..j + k].to_owned()));
            rest = &r[j + k + 1..];
            continue;
        }
        out.push_str(&r[..1]);
        rest = &r[1..];
    }
    out.push_str(rest);
    (out, links)
}
/// Every link target in a document.
pub fn links(src: &str) -> Vec<String> {
    let mut v = vec![];
    for b in parse(src) {
        let texts: Vec<String> = match b { Block::Para(s) | Block::Heading(_, s) => vec![s], Block::List(_, it) => it, Block::Table(h, r) => h.into_iter().chain(r.into_iter().flatten()).collect(), Block::Code(..) => vec![] };
        for t in texts { v.extend(inline(&t).1.into_iter().map(|l| l.1)); }
    }
    v
}

/// The document as elements: blocks keyed `{key}/b{i}`, headings `{key}/h{i}` (the outline: level,
/// title, key). `go` turns a link target into a message.
pub fn render<M: Clone>(src: &str, key: &str, width: f32, go: &dyn Fn(&str) -> M) -> (El<M>, Vec<(u8, String, String)>) {
    let mut c = col().key(key.to_owned()).gap(14.0).width(width);
    let mut outline = vec![];
    for (i, b) in parse(src).into_iter().enumerate() {
        let k = format!("{key}/b{i}");
        let el = match b {
            Block::Heading(n, s) => {
                let hk = format!("{key}/h{i}");
                outline.push((n, s.clone(), hk.clone()));
                let size = match n { 1 => 30.0, 2 => 21.0, _ => 17.0 };
                col().key(k).pad(0.0, if n == 1 { 0.0 } else { 6.0 }).child(text(inline(&s).0, size, Paint::INK).font(Font::Display).key(hk).heading(n))
            }
            Block::Para(s) => para(&k, &s, width, go),
            Block::List(ordered, items) => {
                let mut l = col().key(k.clone()).gap(6.0);
                for (j, it) in items.iter().enumerate() {
                    let mark = if ordered { format!("{}.", j + 1) } else { "·".into() };
                    l = l.child(row().key(format!("{k}/i{j}")).gap(10.0).align(AlignItems::START).child(text(mark, 15.0, Paint::DIM).width(18.0)).child(para(&format!("{k}/{j}/t"), it, width - 28.0, go)));
                }
                l
            }
            Block::Code(_, code) => {
                let mut b = col().key(k.clone()).fill_width().pad(16.0, 12.0).radius(6.0).border(1.0, Paint::FAINT).gap(0.0).selectable();
                for (j, line) in code.lines().enumerate() { b = b.child(text(if line.is_empty() { " ".to_owned() } else { line.to_owned() }, 13.0, Paint::TEXT).font(Font::Mono).key(format!("{k}/l{j}"))); }
                b
            }
            Block::Table(head, rows) => {
                let n = head.len().max(1);
                let cw = width / n as f32;
                let mut t = col().key(k.clone()).role(Role::Table).fill_width();
                let mut h = row().key(format!("{k}/head")).role(Role::Row).pad(0.0, 6.0);
                for c in &head { h = h.child(text(inline(c).0, 13.0, Paint::DIM).width(cw - 8.0).role(Role::ColumnHeader)); }
                t = t.child(h).child(space(0.0, 1.0).fill_width().fill(Paint::DIM));
                for (j, r) in rows.iter().enumerate() {
                    let mut rr = row().key(format!("{k}/r{j}")).role(Role::Row).pad(0.0, 6.0);
                    for c in r { rr = rr.child(text_lh(inline(c).0, 14.0, 22.0, Paint::TEXT).width(cw - 8.0).role(Role::Cell)); }
                    t = t.child(rr).child(space(0.0, 1.0).fill_width().fill(Paint::FAINT));
                }
                t
            }
        };
        c = c.child(el);
    }
    (c, outline)
}
fn para<M: Clone>(k: &str, s: &str, width: f32, go: &dyn Fn(&str) -> M) -> El<M> {
    let (t, links) = inline(s);
    let body = text_lh(t, 15.0, 26.0, Paint::TEXT).width(width).key(format!("{k}/text")).selectable();
    if links.is_empty() { return body; }
    let mut r = row().key(format!("{k}/links")).wrap().gap(10.0);
    for (j, (label, target)) in links.iter().enumerate() {
        r = r.child(text(format!("→ {label}"), 13.0, Paint::ACCENT).key(format!("{k}/link{j}")).on_press(go(target)).focusable(true).role(Role::Link));
    }
    col().key(k.to_owned()).gap(6.0).child(body).child(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_the_subset() {
        let src = "# Title\n\nOne line\nand two with `code` and a [link](#button).\n\n- a\n- b\n\n1. x\n2. y\n\n```rust\nfn f() {}\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n";
        let b = parse(src);
        assert_eq!(b[0], Block::Heading(1, "Title".into()));
        assert_eq!(inline(match &b[1] { Block::Para(s) => s, _ => panic!() }), ("One line and two with code and a link.".into(), vec![("link".into(), "#button".into())]));
        assert_eq!(b[2], Block::List(false, vec!["a".into(), "b".into()]));
        assert_eq!(b[3], Block::List(true, vec!["x".into(), "y".into()]));
        assert_eq!(b[4], Block::Code("rust".into(), "fn f() {}".into()));
        assert_eq!(b[5], Block::Table(vec!["a".into(), "b".into()], vec![vec!["1".into(), "2".into()]]));
        assert_eq!(links(src), vec!["#button".to_owned()]);
    }
}
