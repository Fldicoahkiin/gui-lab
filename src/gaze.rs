//! The annotation layer ("gaze", DESIGN.md §1.7): which laid-out elements get a frame, a number, a
//! size or a callout, and where callout labels go. Plain functions over the layout, no GPU, so all of
//! it is tested headless. The runtime (`Ui`) turns the records into instances for stage 3.
use crate::style::{Gaze, GazeMode};
use crate::ui::{Node, Rect};

/// One annotated element: its node, its real rect (logical px), its running number in tree order
/// (1-based), its kind and its explicit callout, if any.
#[derive(Clone, Debug, PartialEq)]
pub struct Annot { pub node: usize, pub key: String, pub rect: Rect, pub index: u32, pub kind: &'static str, pub label: Option<String> }

/// What the scope-wide readouts drew this frame (DESIGN.md §4.2, §4.3), for tests and the tree.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Readout {
    /// The scope they read (logical px).
    pub scope: Rect,
    /// Edge notes (`EDGE`): each note's text, and where its rotated box landed (logical px).
    pub notes: Vec<(String, Rect)>,
    /// The hairline's column (device px, its left edge) and the pointer's mark on it (logical y).
    pub edge_x: Option<f32>,
    pub mark_y: Option<f32>,
    /// The waveform (`OSC`): the row of nodes it reads and one point per column (logical px).
    pub osc_row: usize,
    pub osc: Vec<[f32; 2]>,
    /// The mean energy (`LEVEL`) and the bar's height (logical px).
    pub level: f32,
    pub level_h: f32,
}

/// Where the waveform's points go: x of each column of nodes inside `[x0, x1]`, the base line `base`,
/// each point `e × amp` above it (logical px).
pub fn osc_points(energy: impl Fn(usize) -> f32, cols: usize, cell: f32, x0: f32, x1: f32, base: f32, amp: f32) -> Vec<[f32; 2]> {
    (0..cols).map(|i| i as f32 * cell).enumerate().filter(|(_, x)| *x >= x0 && *x <= x1).map(|(i, x)| [x, base - energy(i) * amp]).collect()
}
/// The level bar's height: the scope's height × min(1, mean / full).
pub fn level_height(h: f32, level: f32, full: f32) -> f32 { h * (level / full.max(1e-4)).min(1.0) }

/// The node and its ancestors, the node first.
pub fn chain<M>(nodes: &[Node<M>], mut i: usize) -> Vec<usize> {
    let mut v = vec![i];
    while let Some(p) = nodes[i].parent { v.push(p); i = p; }
    v
}

/// The deepest drawn node under a point (logical px).
pub fn under<M>(nodes: &[Node<M>], x: f32, y: f32) -> Option<usize> {
    (0..nodes.len()).rev().find(|&i| !nodes[i].hidden && nodes[i].rect.contains(x, y) && nodes[i].clip.contains(x, y))
}

/// What the annotation layer looks at: the union of the nodes marked as its scope, or the window.
pub fn scope<M>(nodes: &[Node<M>], window: Rect) -> (Rect, Vec<usize>) {
    let marked: Vec<usize> = (0..nodes.len()).filter(|&i| nodes[i].scope && !nodes[i].hidden).collect();
    if marked.is_empty() { return (window, marked); }
    let mut r = nodes[marked[0]].rect.intersect(&nodes[marked[0]].clip);
    for &i in &marked[1..] {
        let o = nodes[i].rect.intersect(&nodes[i].clip);
        let (x0, y0) = (r.x.min(o.x), r.y.min(o.y));
        r = Rect::new(x0, y0, r.right().max(o.right()) - x0, r.bottom().max(o.bottom()) - y0);
    }
    (r, marked)
}

/// Whether a node can be annotated at all: drawn, visible, not tiny, not the root, not a plain wrapper
/// of the same size as its parent, and inside the scope when there is one.
fn candidate<M>(nodes: &[Node<M>], i: usize, scopes: &[usize], only_marked: bool) -> bool {
    let n = &nodes[i];
    let Some(p) = n.parent else { return false };
    if n.hidden || (only_marked && n.annotate.is_none()) { return false; }
    let seen = n.rect.intersect(&n.clip);
    if seen.w < 6.0 || seen.h < 6.0 || seen.w * seen.h < n.rect.w * n.rect.h * 0.5 { return false; }
    if n.annotate.is_none() && n.rect == nodes[p].rect { return false; }
    if !scopes.is_empty() && !scopes.iter().any(|&s| s != i && is_within(nodes, i, s)) && !scopes.contains(&i) { return false; }
    true
}

fn is_within<M>(nodes: &[Node<M>], mut i: usize, root: usize) -> bool {
    loop {
        if i == root { return true; }
        match nodes[i].parent { Some(p) => i = p, None => return false }
    }
}

/// The elements to annotate for a mode: every candidate (all), the element under the pointer and its
/// ancestors (hover), the focused element and its ancestors (focus). At most `density` of them, the
/// largest first; numbered in tree order.
pub fn select<M>(nodes: &[Node<M>], gaze: &Gaze, pointer: Option<(f32, f32)>, focus: Option<usize>, window: Rect) -> Vec<Annot> {
    let (_, scopes) = scope(nodes, window);
    let pool: Vec<usize> = match gaze.mode {
        GazeMode::Off => return vec![],
        GazeMode::All => (0..nodes.len()).collect(),
        GazeMode::Hover => pointer.and_then(|(x, y)| under(nodes, x, y)).map(|i| chain(nodes, i)).unwrap_or_default(),
        GazeMode::Focus => focus.map(|i| chain(nodes, i)).unwrap_or_default(),
    };
    let mut picked: Vec<usize> = pool.into_iter().filter(|&i| candidate(nodes, i, &scopes, gaze.only_marked)).collect();
    // Largest first, explicit callouts before everything else, then back into tree order.
    picked.sort_by(|&a, &b| {
        let (ra, rb) = (nodes[a].rect, nodes[b].rect);
        (nodes[b].annotate.is_some()).cmp(&nodes[a].annotate.is_some()).then((rb.w * rb.h).total_cmp(&(ra.w * ra.h))).then(a.cmp(&b))
    });
    picked.truncate(gaze.density.max(0.0) as usize);
    picked.sort();
    picked.into_iter().enumerate().map(|(k, i)| Annot {
        node: i, key: nodes[i].key.clone(), rect: nodes[i].rect, index: k as u32 + 1, kind: nodes[i].kind(), label: nodes[i].annotate.clone(),
    }).collect()
}

/// Greedy vertical packing of labels in one column: each wants to be centred on `want` and is `h`
/// tall; they keep their order, are at least `gap` apart and stay between `top` and `bottom` when they
/// fit. Returns each label's top, in the order given.
pub fn pack(items: &[(f32, f32)], top: f32, bottom: f32, gap: f32) -> Vec<f32> {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|&a, &b| items[a].0.total_cmp(&items[b].0).then(a.cmp(&b)));
    let mut tops = vec![0.0; items.len()];
    let mut next = top;
    for &i in &order {
        let (want, h) = items[i];
        let y = (want - h * 0.5).max(next);
        tops[i] = y;
        next = y + h + gap;
    }
    // Overflowing the bottom: push back up from the last one, never past the top.
    let mut limit = bottom;
    for &i in order.iter().rev() {
        let h = items[i].1;
        if tops[i] + h > limit { tops[i] = (limit - h).max(top); }
        limit = tops[i] - gap;
    }
    tops
}

/// Where a callout label of an element goes: the margin column on its nearer side of the scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side { Left, Right }
pub fn side(r: Rect, scope: Rect) -> Side { if r.center().0 < scope.center().0 { Side::Left } else { Side::Right } }

/// Labels for the leaders: for each annotated element, its label box (logical px) in its margin
/// column (inside the scope's left or right edge), packed so that none overlap. `size` gives a
/// label's width and height.
pub fn place_labels(annots: &[Annot], scope: Rect, size: impl Fn(&Annot) -> (f32, f32)) -> Vec<(Side, Rect)> {
    let sizes: Vec<(f32, f32)> = annots.iter().map(&size).collect();
    let sides: Vec<Side> = annots.iter().map(|a| side(a.rect, scope)).collect();
    let mut out = vec![(Side::Left, Rect::default()); annots.len()];
    for which in [Side::Left, Side::Right] {
        let idx: Vec<usize> = (0..annots.len()).filter(|&i| sides[i] == which).collect();
        let items: Vec<(f32, f32)> = idx.iter().map(|&i| (annots[i].rect.center().1, sizes[i].1)).collect();
        let tops = pack(&items, scope.y + 8.0, scope.bottom() - 8.0, 6.0);
        for (k, &i) in idx.iter().enumerate() {
            let (w, h) = sizes[i];
            let x = match which { Side::Left => scope.x + 8.0, Side::Right => scope.right() - 8.0 - w };
            out[i] = (which, Rect::new(x, tops[k], w, h));
        }
    }
    out
}

/// How far a frame has drawn on, 0..1, `age` seconds after its element appeared.
pub fn drawn(age: f32, draw_ms: f32) -> f32 { if draw_ms <= 0.0 { 1.0 } else { (age * 1000.0 / draw_ms).clamp(0.0, 1.0) } }
/// A number counting up to `v` as the frame draws on.
pub fn count(v: f32, progress: f32) -> f32 { (v * (1.0 - (1.0 - progress).powi(3))).round() }

/// Where a label `size` (w, h logical px) goes beside what it labels, `ink` (the element's true ink, or
/// its box): above its top left, then below it, then to its right; the first place inside `view` that
/// meets no box in `avoid` (the ink of text to read) and no label already placed (`taken`). `None` when
/// every place would cover something: the label is left out (the frame is drawn without it).
pub fn place_tag(ink: Rect, size: (f32, f32), avoid: &[Rect], taken: &[Rect], view: Rect) -> Option<Rect> {
    let (w, h) = size;
    let tries = [Rect::new(ink.x, ink.y - h - 2.0, w, h), Rect::new(ink.x, ink.bottom() + 3.0, w, h), Rect::new(ink.right() + 4.0, ink.y, w, h)];
    let meets = |a: &Rect, b: &Rect| a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom();
    tries.into_iter().find(|r| r.x >= view.x && r.y >= view.y && r.right() <= view.right() && r.bottom() <= view.bottom()
        && !avoid.iter().any(|a| meets(r, a)) && !taken.iter().any(|t| meets(r, t)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_tag_takes_the_first_free_place_and_none_when_there_is_none() {
        let view = Rect::new(0.0, 0.0, 400.0, 300.0);
        let ink = Rect::new(100.0, 100.0, 120.0, 20.0);
        // Free above: above.
        let r = place_tag(ink, (30.0, 14.0), &[ink], &[], view).expect("a place");
        assert_eq!((r.x, r.y), (100.0, 84.0));
        // Text above it: below.
        let above = Rect::new(90.0, 70.0, 200.0, 20.0);
        let r = place_tag(ink, (30.0, 14.0), &[ink, above], &[], view).expect("a place");
        assert_eq!((r.x, r.y), (100.0, 123.0));
        // Text above and below: to the right.
        let below = Rect::new(90.0, 122.0, 200.0, 20.0);
        let r = place_tag(ink, (30.0, 14.0), &[ink, above, below], &[], view).expect("a place");
        assert_eq!((r.x, r.y), (224.0, 100.0));
        // And a tag already there: nowhere.
        assert!(place_tag(ink, (30.0, 14.0), &[ink, above, below], &[r], view).is_none());
        // At the top of the view, not above it.
        let top = Rect::new(10.0, 2.0, 50.0, 16.0);
        assert_eq!(place_tag(top, (30.0, 14.0), &[top], &[], view).map(|r| r.y), Some(21.0));
    }
}
