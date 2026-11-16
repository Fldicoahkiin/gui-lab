//! The decode transition (DESIGN.md §1.28): an element appears through blocks that resolve from coarse
//! to fine. Its true outline is sampled into lattice cells, as a morph loosens an element into tiles
//! (`morph::cells`); the cells of each tier (32, 16, 8, 4 px…) are the finest cells grouped on a
//! coarser lattice, so every tier covers all the ink. Tier i's cells flip on, each at its own hashed
//! moment of the tier's window; a cell goes when the last of its children is on; once every tier is
//! on, the content is drawn beneath and the finest cells flip off over it. Played backwards, it is the
//! disappearance. At rest (progress 0 or 1) nothing is covered. No GPU here.
use crate::field::hash;
use serde::{Deserialize, Serialize};

/// The transition's numbers: cell sizes (logical px, coarse to fine, up to 6), how far apart the tiers
/// start, how long it all takes, and the seed of the flips. `hold` keeps it at one progress (0..1)
/// instead of playing (a scrubbed timeline).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Decode { pub tiers: Vec<f32>, pub stagger_ms: f32, pub duration_ms: f32, pub seed: u32, #[serde(skip)] pub hold: Option<f32> }
impl Default for Decode { fn default() -> Self { Self { tiers: vec![32.0, 16.0, 8.0, 4.0], stagger_ms: 90.0, duration_ms: 720.0, seed: 7, hold: None } } }

/// One block: its box (device px, x, y, size), its tier, when it is on and off (progress 0..1), and
/// which of the two accent hues it takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Block { pub x: f32, pub y: f32, pub size: f32, pub tier: u8, pub on: f32, pub off: f32, pub hue: u8 }

/// A planned transition: every tier's blocks, and when the content is drawn beneath them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan { pub blocks: Vec<Block>, pub content: f32 }

impl Decode {
    /// Each tier's window (progress): tier i flips on over `[i·S, i·S + W]`, the content comes at
    /// `C = (T−1)·S + W`, the finest flip off over `[C, C + W]`, with `W = (D − (T−1)·S) / 2`.
    pub fn windows(&self) -> (f32, f32, f32) {
        let d = self.duration_ms.max(1.0);
        let t = self.tiers.len().max(1) as f32;
        let s = self.stagger_ms.clamp(0.0, d / t);
        let w = ((d - (t - 1.0) * s) * 0.5).max(1.0);
        (s / d, w / d, ((t - 1.0) * s + w) / d)
    }
    /// Plan it over the finest cells that hold ink: `cells` are their top-left corners (device px) on
    /// the lattice of the last tier, `scale` device px per logical px.
    pub fn plan(&self, cells: &[(f32, f32)], scale: f32) -> Plan {
        let (s, w, c) = self.windows();
        let sizes: Vec<f32> = self.tiers.iter().take(6).map(|t| (t * scale).round().max(1.0)).collect();
        let n = sizes.len();
        let mut blocks = vec![];
        // Tier by tier, the distinct cells of that size holding a finest cell's centre.
        let mut index: Vec<std::collections::HashMap<(i64, i64), usize>> = vec![Default::default(); n];
        let fine = *sizes.last().unwrap_or(&4.0);
        for &(x, y) in cells {
            let (cx, cy) = (x + fine * 0.5, y + fine * 0.5);
            for (ti, &sz) in sizes.iter().enumerate() {
                let key = ((cx / sz).floor() as i64, (cy / sz).floor() as i64);
                if index[ti].contains_key(&key) { continue; }
                let h = hash(self.seed ^ (ti as u32).wrapping_mul(0x9E37) ^ (key.0 as u32).wrapping_mul(73_856_093) ^ (key.1 as u32).wrapping_mul(19_349_663));
                index[ti].insert(key, blocks.len());
                blocks.push(Block { x: key.0 as f32 * sz, y: key.1 as f32 * sz, size: sz, tier: ti as u8, on: ti as f32 * s + w * h, off: 2.0, hue: (hash(h.to_bits()) > 0.5) as u8 });
            }
        }
        // A block goes when the last of its children is on; the finest go over the content.
        for ti in (0..n).rev() {
            let keys: Vec<((i64, i64), usize)> = index[ti].iter().map(|(k, v)| (*k, *v)).collect();
            for (key, b) in keys {
                if ti + 1 == n {
                    let h = hash(self.seed.wrapping_add(991) ^ (key.0 as u32).wrapping_mul(83_492_791) ^ (key.1 as u32).wrapping_mul(2_654_435_761));
                    blocks[b].off = c + w * h;
                } else {
                    let r = (sizes[ti] / sizes[ti + 1]).round().max(1.0) as i64;
                    let last = (0..r).flat_map(|dy| (0..r).map(move |dx| (dx, dy)))
                        .filter_map(|(dx, dy)| index[ti + 1].get(&(key.0 * r + dx, key.1 * r + dy))).map(|&k| blocks[k].on).fold(0.0, f32::max);
                    blocks[b].off = last.max(blocks[b].on);
                }
            }
        }
        Plan { blocks, content: c }
    }
}
impl Plan {
    /// The blocks showing at progress `u`, coarse first (finer ones are drawn over them).
    pub fn at(&self, u: f32) -> impl Iterator<Item = &Block> { self.blocks.iter().filter(move |b| u >= b.on && u < b.off) }
    /// Whether the content is drawn at progress `u`.
    pub fn shows_content(&self, u: f32) -> bool { u >= self.content }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn square() -> Vec<(f32, f32)> { (0..20).flat_map(|j| (0..30).map(move |i| (i as f32 * 4.0, j as f32 * 4.0))).collect() }
    #[test]
    fn tiers_cover_the_ink_until_the_content_and_nothing_is_left_at_rest() {
        let d = Decode::default();
        let p = d.plan(&square(), 1.0);
        assert_eq!(p.at(0.0).count(), 0);
        assert_eq!(p.at(1.0).count(), 0, "at rest nothing is half covered");
        // From the moment every tier is on until the content shows, every finest cell is under a block.
        for k in 0..40 {
            let u = p.content * 0.999 * k as f32 / 39.0;
            if u < d.windows().1 { continue; }
            for &(x, y) in &square() {
                let covered = p.at(u).any(|b| x + 2.0 >= b.x && x + 2.0 < b.x + b.size && y + 2.0 >= b.y && y + 2.0 < b.y + b.size);
                assert!(covered, "cell ({x}, {y}) open at {u}");
            }
        }
        // Coarse to fine: the mean size shown falls.
        let mean = |u: f32| { let v: Vec<f32> = p.at(u).map(|b| b.size).collect(); v.iter().sum::<f32>() / v.len().max(1) as f32 };
        assert!(mean(0.2) > mean(p.content - 0.01));
        // Deterministic.
        assert_eq!(d.plan(&square(), 1.0), p);
    }
}
