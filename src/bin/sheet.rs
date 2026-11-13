//! Contact sheets of the picture marks: one PNG per mark set, plus palettes, relief, regions, follow,
//! glow, parallax and the compare wipe, each over the three detailed demo pictures, fully loosened.
//! Also writes the mark atlas itself and the raw pictures.
//! usage: sheet <out_dir>
use lab::demo::{self, BUST, PICTURE_SIZES, SKY, TERRAIN};
use lab::el::{El, col, picture, row};
use lab::marks::{self, Atlas};
use lab::offscreen::{Offscreen, save_png};
use lab::style::{DARK, Look, Material, PicFx, colour, set};
use lab::ui::Ui;

const W: u32 = 1500;
const H: u32 = 620;

/// Two rows of the three pictures, fully loosened: row 1 uses slot 0, row 2 slot 1. With `wipe`,
/// nothing is loosened at rest and the bust of row 2 gets the compare line (the field holds one).
struct Sheet { wipe: Option<f32> }

fn view(s: &Sheet) -> El<()> {
    let line = |slot: u8, r: usize| {
        let mut out = row().gap(12.0);
        for id in [SKY, TERRAIN, BUST] {
            let (w, h) = PICTURE_SIZES[id as usize];
            let mut p = picture(id, h as f32 / w as f32).pic_fx(slot).key(format!("p{r}/{id}")).grow(1.0);
            match s.wipe {
                Some(x) if r == 1 && id == BUST => p = p.wipe(x),
                Some(_) => {}
                None => p = p.loose(1.0),
            }
            out = out.child(p);
        }
        out
    };
    col().pad(12.0, 12.0).gap(12.0).child(line(0, 0)).child(line(1, 1))
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "sheets".into());
    std::fs::create_dir_all(&out).expect("out dir");
    let mut o = Offscreen::new(W, H);
    let faces = lab::text::load_bundled("assets").expect("fonts");
    let mut ui = Ui::new(Sheet { wipe: None }, view, |_, _| {}, faces, W, H, 1.0);
    ui.tick(100.0);

    // The atlas, enlarged 2×, mark in white and height in the second half.
    let atlas = Atlas::build(ui.text_mut(), lab::text::MONO);
    let (aw, ah) = (marks::WIDTH as u32, marks::HEIGHT as u32);
    let mut img = vec![0u8; (aw * 2 * ah * 2 * 4) as usize * 2];
    for y in 0..ah * 2 { for x in 0..aw * 2 { for half in 0..2u32 {
        let o = (((y + half * ah * 2) * aw * 2 + x) * 4) as usize;
        let v = atlas.pixels[(((y / 2) * aw + x / 2) * 2 + half) as usize];
        let grid = (x / 2) % 32 == 0 || (y / 2) % 32 == 0;
        img[o..o + 4].copy_from_slice(&if grid { [90, 30, 20, 255] } else { [v, v, v, 255] });
    } } }
    save_png(&format!("{out}/atlas.png"), aw * 2, ah * 4, &img);
    eprintln!("characters by ink: {:?}", atlas.chars.iter().collect::<String>());
    for (id, w, h, px) in demo::pictures() { save_png(&format!("{out}/picture-{id}.png"), w, h, &px); }

    let mut shoot = |ui: &mut Ui<Sheet, ()>, name: &str, a: PicFx, b: PicFx| {
        let mut look = Look { theme: DARK, material: Material::Plain, ..Look::default() };
        look.pics[0] = a; look.pics[1] = b;
        let px = o.render(ui, &look);
        save_png(&format!("{out}/{name}.png"), W, H, &px);
        eprintln!("{name}");
    };

    // Per set: the same set at two pitches, with values that suit it.
    for s in 0..set::COUNT as u8 {
        let base = match s {
            set::CHARS => PicFx { pitch: 10.0, levels: 16.0, ground: 0.0, gamma: 0.6, ..PicFx::with_set(s) },
            set::DITHER => PicFx { pitch: 3.0, levels: 16.0, ground: 0.0, colour: colour::THEME_INK as f32, ..PicFx::with_set(s) },
            set::PIXEL => PicFx { pitch: 6.0, ..PicFx::with_set(s) },
            set::MOSAIC | set::BRICK => PicFx { pitch: 10.0, relief: 1.0, ground: 0.2, ..PicFx::with_set(s) },
            set::GLOW => PicFx { pitch: 8.0, levels: 12.0, ground: 0.0, glow: 1.0, gamma: 0.8, ..PicFx::with_set(s) },
            set::STROKE => PicFx { pitch: 9.0, levels: 8.0, ground: 0.15, follow: 1.0, ..PicFx::with_set(s) },
            _ => PicFx { pitch: 8.0, levels: 8.0, ground: 0.15, ..PicFx::with_set(s) },
        };
        let big = PicFx { pitch: base.pitch * 2.0, ..base };
        shoot(&mut ui, &format!("set-{s:02}-{}", ["chars", "dither", "pixel", "mosaic", "brick", "dots", "cross", "diagonal", "diamond", "lines", "glow", "stroke"][s as usize]), base, big);
    }
    // Palettes: blocky tone-quantised posters (solid pixels, dithered hand-over), and dither marks.
    for (i, (name, n, _)) in marks::PALETTES.iter().enumerate() {
        let poster = PicFx { pitch: 6.0, set: set::PIXEL as f32, palette: i as f32, colours: *n as f32, spread: 0.5, ..PicFx::default() };
        let dith = PicFx { pitch: 3.0, set: set::DITHER as f32, levels: 16.0, palette: i as f32, colours: *n as f32, spread: 0.35, ground: 0.0, ..PicFx::default() };
        shoot(&mut ui, &format!("palette-{i}-{n}"), poster, dith);
        eprintln!("  ({name})");
    }
    // Relief on and off, mosaic and brick.
    shoot(&mut ui, "relief-mosaic", PicFx { pitch: 12.0, relief: 0.0, ..PicFx::with_set(set::MOSAIC) }, PicFx { pitch: 12.0, relief: 1.5, ..PicFx::with_set(set::MOSAIC) });
    shoot(&mut ui, "relief-brick", PicFx { pitch: 14.0, relief: 0.0, ..PicFx::with_set(set::BRICK) }, PicFx { pitch: 14.0, relief: 1.5, ..PicFx::with_set(set::BRICK) });
    // Regions: by hash, then by brightness band.
    let reg = PicFx { pitch: 7.0, levels: 12.0, ground: 0.05, regions: 6.0, region_count: 4.0, ..PicFx::default() };
    shoot(&mut ui, "regions", PicFx { region_by: 0.0, ..reg }, PicFx { region_by: 1.0, ..reg });
    // Follow: strokes unturned, then along the contours.
    let st = PicFx { pitch: 10.0, levels: 8.0, ground: 0.1, ..PicFx::with_set(set::STROKE) };
    shoot(&mut ui, "follow", PicFx { follow: 0.0, ..st }, PicFx { follow: 1.0, ..st });
    // Colour modes on characters: theme ink, then two colours by brightness.
    let ch = PicFx { pitch: 10.0, levels: 16.0, ground: 0.0, gamma: 0.6, ..PicFx::with_set(set::CHARS) };
    shoot(&mut ui, "colour-modes", PicFx { colour: colour::THEME_INK as f32, ..ch }, PicFx { colour: colour::TWO as f32, tint_a: [0.1, 0.3, 0.9], tint_b: [1.0, 0.85, 0.3], ..ch });
    // Glow off and on.
    let gl = PicFx { pitch: 8.0, levels: 12.0, ground: 0.0, gamma: 0.8, ..PicFx::with_set(set::GLOW) };
    shoot(&mut ui, "glow", PicFx { glow: 0.0, ..gl }, PicFx { glow: 1.5, ..gl });
    // Ordered spread between levels: hard, then dithered.
    let sp = PicFx { pitch: 6.0, levels: 4.0, ground: 0.0, ..PicFx::with_set(set::DOTS) };
    shoot(&mut ui, "spread", PicFx { spread: 0.0, ..sp }, PicFx { spread: 1.0, ..sp });
    // Parallax toward a pointer in the middle of the sheet.
    ui.pointer_move(W as f32 * 0.5, H as f32 * 0.5);
    shoot(&mut ui, "depth", PicFx { depth: 0.0, ..ch }, PicFx { depth: 12.0, ..ch });
    ui.pointer_leave();
    // The compare wipe, left original, right loosened.
    ui.state.wipe = Some(0.5);
    ui.relayout();
    shoot(&mut ui, "wipe", ch, PicFx { pitch: 5.0, ..PicFx::with_set(set::DITHER) });
}
