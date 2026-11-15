//! Sheets of the film (DESIGN.md §6), drawn off-screen with a fixed 60 Hz clock: every chapter at rest
//! with its stage at 2390 × 1000 device px (`stills.png`), five frames across each of the thirteen cuts
//! (`cuts.png`), four frames of each chapter's own interaction (`interactions.png`), and the window at
//! 1280 × 800 and 800 × 1200 with the stage letterboxed (`window-1280x800.png`, `window-800x1200.png`).
//!
//! usage: reel <out_dir> [stills | cuts | interactions | window | one <chapter>]   (all of them when omitted)
use lab::el::Key;
use lab::gallery::film::{self, FMsg};
use lab::gallery::{self, GalleryUi, Msg, Page};
use lab::offscreen::{Offscreen, crop, save_png, sheet};

/// The window for stills: logical size and scale such that the stage is 2390 × 1000 device px.
const SW: u32 = 1690;
const SH: u32 = 840;
const SCALE: f32 = 1.5;

fn faces() -> Vec<lab::text::Face> { lab::text::load_bundled("assets").expect("fonts") }
fn open(w: u32, h: u32, scale: f32) -> GalleryUi {
    let mut ui = gallery::new_ui(faces(), (w as f32 * scale) as u32, (h as f32 * scale) as u32, scale);
    ui.pointer_leave();
    gallery::update(&mut ui.state, Msg::Go(Page::Film));
    ui.relayout();
    ui
}
/// Tick at 60 Hz to `to`, drawing every frame (the renderer keeps the last one for commits).
fn run(o: &mut Offscreen, ui: &mut GalleryUi, to: f32) -> Vec<u8> {
    let mut t = ui.now();
    let mut px = vec![];
    while t < to - 1e-4 {
        t = (t + 1.0 / 60.0).min(to);
        ui.tick(t);
        if ui.needs_frame(t) || px.is_empty() { let look = ui.state.look(); px = o.render(ui, &look); }
    }
    if px.is_empty() { let look = ui.state.look(); px = o.render(ui, &look); }
    px
}
/// The stage's device rect.
fn stage(ui: &GalleryUi, scale: f32) -> [u32; 4] {
    let g = film::geo(ui.state.width, ui.state.height);
    [(g.sx * scale).round() as u32, (g.sy * scale).round() as u32, (g.sw * scale).round() as u32, (g.sh * scale).round() as u32]
}
fn stage_of(o: &Offscreen, ui: &GalleryUi, px: &[u8], scale: f32) -> (u32, u32, Vec<u8>) { crop(px, o.w, o.h, stage(ui, scale)) }
fn send(ui: &mut GalleryUi, m: FMsg) { gallery::update(&mut ui.state, Msg::Film(m)); ui.relayout(); }
/// Paused, in chapter `c`, settled.
fn at_rest(o: &mut Offscreen, ui: &mut GalleryUi, c: usize) {
    if ui.state.film.playing { send(ui, FMsg::Play); }
    film::go(&mut ui.state, c, None);
    ui.relayout();
    let t = ui.now() + 3.0;
    run(o, ui, t);
}
/// Half size, for sheets that would be too large.
fn half((w, h, px): (u32, u32, Vec<u8>)) -> (u32, u32, Vec<u8>) {
    let (hw, hh) = (w / 2, h / 2);
    let mut out = vec![0u8; (hw * hh * 4) as usize];
    for y in 0..hh { for x in 0..hw { for c in 0..4 {
        let s: u32 = [(0, 0), (1, 0), (0, 1), (1, 1)].iter().map(|(dx, dy)| px[(((y * 2 + dy) * w + x * 2 + dx) * 4 + c) as usize] as u32).sum();
        out[((y * hw + x) * 4 + c) as usize] = (s / 4) as u8;
    } } }
    (hw, hh, out)
}

fn stills(out: &str) {
    let mut o = Offscreen::new((SW as f32 * SCALE) as u32, (SH as f32 * SCALE) as u32);
    let mut ui = open(SW, SH, SCALE);
    let mut frames = vec![];
    for c in 0..film::CHAPTERS {
        at_rest(&mut o, &mut ui, c);
        let px = { let t = ui.now() + 0.05; run(&mut o, &mut ui, t) };
        let f = stage_of(&o, &ui, &px, SCALE);
        save_png(&format!("{out}/still-{:02}.png", c + 1), f.0, f.1, &f.2);
        eprintln!("still {:02} {}", c + 1, film::CH[c].name);
        frames.push(f);
    }
    let (w, h, px) = sheet(&frames, 2);
    save_png(&format!("{out}/stills.png"), w, h, &px);
}

/// Five frames across every cut (chapter c → c + 1, the last back to the first): just before, three
/// through the target's transition, and after it has settled.
fn cuts(out: &str) {
    let (w, h, s) = (1280u32, 640u32, 1.0);
    let mut o = Offscreen::new(w, h);
    let mut ui = open(w, h, s);
    let mut rows = vec![];
    for c in 0..film::CHAPTERS {
        at_rest(&mut o, &mut ui, c);
        let before = { let t = ui.now() + 0.05; run(&mut o, &mut ui, t) };
        let mut strip = vec![stage_of(&o, &ui, &before, s)];
        send(&mut ui, FMsg::Next);
        let t0 = ui.now();
        let next = (c + 1) % film::CHAPTERS;
        let d = ui.state.presets[film::CH[next].style].transition.duration_ms.max(300.0) / 1000.0;
        for f in [0.2, 0.45, 0.75] { let px = run(&mut o, &mut ui, t0 + d * f); strip.push(stage_of(&o, &ui, &px, s)); }
        let px = run(&mut o, &mut ui, t0 + d + 1.2);
        strip.push(stage_of(&o, &ui, &px, s));
        let (sw, sh, spx) = sheet(&strip, 5);
        save_png(&format!("{out}/cut-{:02}-{:02}.png", c + 1, next + 1), sw, sh, &spx);
        eprintln!("cut {:02} → {:02}", c + 1, next + 1);
        rows.push(half((sw, sh, spx)));
    }
    let (w, h, px) = sheet(&rows, 1);
    save_png(&format!("{out}/cuts.png"), w, h, &px);
}

/// What each chapter's interaction is, as steps; four frames are taken across it.
fn interactions(out: &str) {
    let (w, h, s) = (1280u32, 640u32, 1.0);
    let mut o = Offscreen::new(w, h);
    let mut ui = open(w, h, s);
    let mut rows = vec![];
    for c in 0..film::CHAPTERS {
        at_rest(&mut o, &mut ui, c);
        let mut strip = vec![];
        let g = film::geo(ui.state.width, ui.state.height);
        let p = |x: f32, y: f32| (g.sx + x * g.k, g.sy + y * g.k);
        let shoot = |o: &mut Offscreen, ui: &mut GalleryUi, dt: f32, strip: &mut Vec<(u32, u32, Vec<u8>)>| {
            let t = ui.now() + dt; let px = run(o, ui, t);
            strip.push(stage_of(o, ui, &px, s));
        };
        let center = |ui: &GalleryUi, k: &str| ui.node(k).map(|n| n.rect.center()).unwrap_or((0.0, 0.0));
        match c {
            0 => {
                shoot(&mut o, &mut ui, 0.05, &mut strip);
                let (x, y) = center(&ui, "film/c01/title"); ui.press(x, y); ui.release(x, y);
                for ch in "，一直".chars() { ui.text_input(&ch.to_string()); }
                shoot(&mut o, &mut ui, 0.1, &mut strip);
                let (x, y) = center(&ui, "film/c01/invert"); ui.press(x, y); ui.release(x, y);
                shoot(&mut o, &mut ui, 0.8, &mut strip);
                let (x, y) = center(&ui, "film/c01/count/more"); ui.press(x, y); ui.release(x, y);
                shoot(&mut o, &mut ui, 0.08, &mut strip);
                // Put the sentence back for the chapters after.
                send(&mut ui, FMsg::Sentence(lab::widgets::TextEdit::new("松开，再合上")));
                let (x, y) = center(&ui, "film/c01/invert"); ui.press(x, y); ui.release(x, y);
            }
            1 => {
                shoot(&mut o, &mut ui, 0.05, &mut strip);
                for ch in "go".chars() { ui.text_input(&ch.to_string()); }
                shoot(&mut o, &mut ui, 0.08, &mut strip);
                ui.text_input("0");
                shoot(&mut o, &mut ui, 0.06, &mut strip);
                ui.key(Key::Down, false);
                shoot(&mut o, &mut ui, 0.5, &mut strip);
                for _ in 0..3 { ui.key(Key::Backspace, false); }
            }
            2 => {
                shoot(&mut o, &mut ui, 0.05, &mut strip);
                let (x, y) = center(&ui, "film/c03/sign");
                ui.press(x, y);
                for i in 1..=10 { ui.pointer_move(x + i as f32 * 8.0, y); { let t = ui.now() + 1.0 / 60.0; run(&mut o, &mut ui, t) }; }
                ui.release(x + 80.0, y);
                shoot(&mut o, &mut ui, 0.05, &mut strip);
                let (x, y) = center(&ui, "film/c03/part2"); ui.pointer_move(x, y);
                shoot(&mut o, &mut ui, 0.4, &mut strip);
                let (x, y) = center(&ui, "film/c03/part3"); ui.pointer_move(x, y);
                shoot(&mut o, &mut ui, 0.4, &mut strip);
            }
            3 => {
                let (x, y) = center(&ui, "film/c04/sentence"); ui.pointer_move(x, y);
                shoot(&mut o, &mut ui, 0.5, &mut strip);
                let (x, y) = center(&ui, "film/c04/sign"); ui.pointer_move(x, y);
                ui.press(x, y); ui.release(x, y);
                shoot(&mut o, &mut ui, 0.25, &mut strip);
                shoot(&mut o, &mut ui, 0.45, &mut strip);
                shoot(&mut o, &mut ui, 1.0, &mut strip);
            }
            4 => {
                shoot(&mut o, &mut ui, 0.05, &mut strip);
                send(&mut ui, FMsg::Play);
                shoot(&mut o, &mut ui, 1.05, &mut strip);
                send(&mut ui, FMsg::Play);
                let (x, y) = center(&ui, "film/c05/sign"); ui.pointer_move(x, y);
                shoot(&mut o, &mut ui, 0.1, &mut strip);
                shoot(&mut o, &mut ui, 0.6, &mut strip);
            }
            5 => {
                shoot(&mut o, &mut ui, 0.05, &mut strip);
                let (x, y) = center(&ui, "film/c06/sign");
                ui.press(x, y);
                for i in 1..=12 { ui.pointer_move(x - i as f32 * 10.0, y + i as f32 * 3.0); { let t = ui.now() + 1.0 / 60.0; run(&mut o, &mut ui, t) }; }
                shoot(&mut o, &mut ui, 0.02, &mut strip);
                for i in 1..=12 { ui.pointer_move(x + i as f32 * 14.0 - 120.0, y + 36.0 - i as f32 * 8.0); { let t = ui.now() + 1.0 / 60.0; run(&mut o, &mut ui, t) }; }
                ui.release(x + 48.0, y - 60.0);
                shoot(&mut o, &mut ui, 0.05, &mut strip);
                send(&mut ui, FMsg::Gap(lab::widgets::Edit::Set(36.0)));
                shoot(&mut o, &mut ui, 0.3, &mut strip);
                send(&mut ui, FMsg::Gap(lab::widgets::Edit::Set(14.0)));
                send(&mut ui, FMsg::Obj(330.0, 40.0));
            }
            6 => {
                shoot(&mut o, &mut ui, 0.05, &mut strip);
                send(&mut ui, FMsg::Form);
                shoot(&mut o, &mut ui, 0.7, &mut strip);
                shoot(&mut o, &mut ui, 0.6, &mut strip);
                shoot(&mut o, &mut ui, 1.4, &mut strip);
            }
            7 => {
                shoot(&mut o, &mut ui, 0.05, &mut strip);
                let (x, y) = p(800.0, 280.0);
                ui.press(x, y);
                shoot(&mut o, &mut ui, 0.2, &mut strip);
                ui.release(x, y);
                let (x, y) = p(600.0, 400.0); ui.press(x, y); ui.release(x, y);
                let (x, y) = p(500.0, 330.0); ui.press(x, y); ui.release(x, y);
                shoot(&mut o, &mut ui, 0.3, &mut strip);
                shoot(&mut o, &mut ui, 0.6, &mut strip);
                send(&mut ui, FMsg::Retry);
            }
            8 => {
                ui.pointer_move(p(300.0, 200.0).0, p(300.0, 200.0).1);
                shoot(&mut o, &mut ui, 0.05, &mut strip);
                ui.pointer_move(p(1300.0, 500.0).0, p(1300.0, 500.0).1);
                shoot(&mut o, &mut ui, 0.05, &mut strip);
                send(&mut ui, FMsg::Explode);
                shoot(&mut o, &mut ui, 0.3, &mut strip);
                shoot(&mut o, &mut ui, 1.0, &mut strip);
                send(&mut ui, FMsg::Explode);
            }
            9 => {
                shoot(&mut o, &mut ui, 0.05, &mut strip);
                send(&mut ui, FMsg::Knob(0.95));
                shoot(&mut o, &mut ui, 0.3, &mut strip);
                send(&mut ui, FMsg::Squares);
                send(&mut ui, FMsg::Plane(1));
                shoot(&mut o, &mut ui, 0.6, &mut strip);
                send(&mut ui, FMsg::Plane(2));
                shoot(&mut o, &mut ui, 0.8, &mut strip);
                send(&mut ui, FMsg::Plane(0)); send(&mut ui, FMsg::Squares); send(&mut ui, FMsg::Knob(0.55));
            }
            10 => {
                shoot(&mut o, &mut ui, 0.05, &mut strip);
                let (x, y) = center(&ui, "film/c11/item2"); ui.pointer_move(x, y);
                shoot(&mut o, &mut ui, 0.3, &mut strip);
                ui.press(x, y); ui.release(x, y);
                shoot(&mut o, &mut ui, 0.2, &mut strip);
                shoot(&mut o, &mut ui, 0.8, &mut strip);
                send(&mut ui, FMsg::MenuSel(1));
            }
            12 => {
                use lab::gallery::mechanism::MMsg;
                shoot(&mut o, &mut ui, 0.05, &mut strip);
                // Turn the train by dragging the big gear, then hover a part, explode, ascii.
                let r = ui.node("film/c13/machine/view").expect("view").rect;
                let (gx, gy) = (r.x + r.w * 0.36, r.y + r.h * 0.52);
                ui.press(gx, gy);
                for i in 1..=10 { ui.pointer_move(gx + i as f32 * 9.0, gy); { let t = ui.now() + 1.0 / 60.0; run(&mut o, &mut ui, t) }; }
                ui.release(gx + 90.0, gy);
                ui.pointer_move(r.x + r.w * 0.6, r.y + r.h * 0.4);
                shoot(&mut o, &mut ui, 0.3, &mut strip);
                send(&mut ui, FMsg::Mech(MMsg::Explode(1.0)));
                shoot(&mut o, &mut ui, 1.0, &mut strip);
                send(&mut ui, FMsg::Mech(MMsg::Material(1)));
                shoot(&mut o, &mut ui, 0.3, &mut strip);
                send(&mut ui, FMsg::Mech(MMsg::Material(0)));
                send(&mut ui, FMsg::Mech(MMsg::Explode(0.0)));
            }
            _ => {
                shoot(&mut o, &mut ui, 0.05, &mut strip);
                send(&mut ui, FMsg::Finale);
                shoot(&mut o, &mut ui, 0.9, &mut strip);
                shoot(&mut o, &mut ui, 0.8, &mut strip);
                shoot(&mut o, &mut ui, 2.2, &mut strip);
            }
        }
        ui.pointer_leave();
        let (sw, sh, spx) = sheet(&strip, 4);
        save_png(&format!("{out}/interaction-{:02}.png", c + 1), sw, sh, &spx);
        eprintln!("interaction {:02} {}", c + 1, film::CH[c].name);
        rows.push(half((sw, sh, spx)));
    }
    let (w, h, px) = sheet(&rows, 1);
    save_png(&format!("{out}/interactions.png"), w, h, &px);
}

fn window(out: &str) {
    for (w, h, c) in [(1280u32, 800u32, 4usize), (800, 1200, 6)] {
        let mut o = Offscreen::new(w, h);
        let mut ui = open(w, h, 1.0);
        at_rest(&mut o, &mut ui, c);
        let px = { let t = ui.now() + 0.05; run(&mut o, &mut ui, t) };
        save_png(&format!("{out}/window-{w}x{h}.png"), w, h, &px);
        eprintln!("window {w}×{h}");
    }
}

fn one(out: &str, c: usize) {
    let (w, h, s) = (1280u32, 640u32, 1.0);
    let mut o = Offscreen::new(w, h);
    let mut ui = open(w, h, s);
    at_rest(&mut o, &mut ui, c);
    let px = { let t = ui.now() + 0.05; run(&mut o, &mut ui, t) };
    save_png(&format!("{out}/one-{:02}.png", c + 1), w, h, &px);
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = a.get(1).cloned().unwrap_or_else(|| "reel".into());
    std::fs::create_dir_all(&out).expect("out dir");
    match a.get(2).map(String::as_str) {
        Some("stills") => stills(&out),
        Some("cuts") => cuts(&out),
        Some("interactions") => interactions(&out),
        Some("window") => window(&out),
        Some("one") => { for c in a[3..].iter().filter_map(|v| v.parse::<usize>().ok()) { one(&out, c.clamp(1, film::CHAPTERS) - 1); } }
        _ => { stills(&out); cuts(&out); interactions(&out); window(&out); }
    }
}
