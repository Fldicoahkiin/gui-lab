//! Render every gallery page in every theme and material to PNG files (one per combination), plus a
//! few states: a dialog open, the narrow layout, a theme switch half-way, typing into an input.
//! usage: pages <out_dir> [width] [height] [scale]     (device px)
use lab::el::Key;
use lab::gallery::{self, GalleryUi, Msg, Page};
use lab::offscreen::{Offscreen, save_png};
use lab::style::MATERIALS;

const THEME_FILE: [&str; 3] = ["dark", "light", "pixel"];

fn settle(ui: &mut GalleryUi, secs: f32) {
    let to = ui.now() + secs;
    let mut t = ui.now();
    while t < to { t = (t + 1.0 / 30.0).min(to); ui.tick(t); }
}

fn shoot(o: &mut Offscreen, ui: &mut GalleryUi, out: &str, name: &str) {
    let look = ui.state.look();
    let px = o.render(ui, &look);
    save_png(&format!("{out}/{name}.png"), o.w, o.h, &px);
}

fn send(ui: &mut GalleryUi, m: Msg) {
    gallery::update(&mut ui.state, m);
    ui.relayout();
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = a.get(1).cloned().unwrap_or_else(|| "pages".into());
    let w: u32 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(1280);
    let h: u32 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(1800);
    let scale: f32 = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(1.0);
    std::fs::create_dir_all(&out).expect("out dir");
    let faces = || lab::text::load_bundled("assets").expect("fonts");
    let mut o = Offscreen::new(w, h);
    let mut ui = gallery::new_ui(faces(), w, h, scale);
    ui.pointer_leave();

    for p in Page::all() {
        for (ti, tname) in THEME_FILE.iter().enumerate() {
            for (mi, (_, mname)) in MATERIALS.iter().enumerate() {
                send(&mut ui, Msg::Go(p));
                send(&mut ui, Msg::Theme(ti));
                send(&mut ui, Msg::Material(mi));
                settle(&mut ui, 3.0);
                shoot(&mut o, &mut ui, &out, &format!("{}-{tname}-{mname}", p.hash()));
            }
        }
        eprintln!("{}", p.hash());
    }

    // States, at window height 900.
    let mut o = Offscreen::new(w, 900);
    let mut ui = gallery::new_ui(faces(), w, 900, scale);
    ui.pointer_leave();
    send(&mut ui, Msg::Go(Page::Comp(9)));
    settle(&mut ui, 3.0);
    let (x, y) = ui.node("dialog/open").expect("open button").rect.center();
    ui.press(x, y);
    ui.release(x, y);
    for c in "Studio".chars() { ui.key(Key::Char(c), false); }
    ui.pointer_leave();
    settle(&mut ui, 0.25);
    shoot(&mut o, &mut ui, &out, "state-dialog-opening");
    settle(&mut ui, 3.0);
    shoot(&mut o, &mut ui, &out, "state-dialog-open-dark-dot");
    send(&mut ui, Msg::Material(0));
    settle(&mut ui, 1.0);
    shoot(&mut o, &mut ui, &out, "state-dialog-open-dark-plain");
    send(&mut ui, Msg::Theme(1));
    settle(&mut ui, 1.0);
    shoot(&mut o, &mut ui, &out, "state-dialog-open-light-plain");
    ui.key(Key::Escape, false);
    settle(&mut ui, 3.0);
    shoot(&mut o, &mut ui, &out, "state-dialog-closed");

    // A theme switch, pressed on the top bar: the old frame beyond the front, the new one inside.
    send(&mut ui, Msg::Theme(0));
    send(&mut ui, Msg::Material(1));
    send(&mut ui, Msg::Go(Page::Comp(0)));
    settle(&mut ui, 3.0);
    // The renderer keeps the look of the frame it last drew as the old one, as a window drawing every
    // frame would.
    shoot(&mut o, &mut ui, &out, "state-theme-before");
    let (x, y) = ui.node("theme:1").expect("theme segment").rect.center();
    ui.press(x, y);
    ui.release(x, y);
    ui.pointer_leave();
    settle(&mut ui, 0.5);
    shoot(&mut o, &mut ui, &out, "state-theme-switching");
    // And the material, from its own control.
    let (x, y) = ui.node("material:0").expect("material segment").rect.center();
    ui.press(x, y);
    ui.release(x, y);
    ui.pointer_leave();
    settle(&mut ui, 0.6);
    shoot(&mut o, &mut ui, &out, "state-material-switching");
    settle(&mut ui, 3.0);
    shoot(&mut o, &mut ui, &out, "state-theme-switched");

    // Navigation from the rail, half-way.
    let (x, y) = ui.node("nav/slider").expect("rail item").rect.center();
    ui.press(x, y);
    ui.release(x, y);
    ui.pointer_leave();
    settle(&mut ui, 0.2);
    shoot(&mut o, &mut ui, &out, "state-navigating");

    // Typing into the input.
    send(&mut ui, Msg::Go(Page::Comp(2)));
    settle(&mut ui, 3.0);
    let (x, y) = ui.node("input/live").expect("input").rect.center();
    ui.press(x, y);
    ui.release(x, y);
    for c in "hello world".chars() { ui.key(Key::Char(c), false); }
    for _ in 0..5 { ui.key(Key::Left, false); }
    ui.pointer_leave();
    settle(&mut ui, 3.0);
    shoot(&mut o, &mut ui, &out, "state-input-typed");

    // Narrow: the rail becomes a row of tabs.
    let mut o = Offscreen::new(600, 1100);
    let mut ui = gallery::new_ui(faces(), 600, 1100, scale);
    ui.pointer_leave();
    for p in [Page::Comp(0), Page::Comp(7), Page::Marks(0)] {
        send(&mut ui, Msg::Go(p));
        settle(&mut ui, 3.0);
        shoot(&mut o, &mut ui, &out, &format!("narrow-{}", p.hash()));
    }
}
