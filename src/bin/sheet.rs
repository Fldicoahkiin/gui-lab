//! Contact sheets of the picture marks: one PNG per mark set, plus palettes, relief, regions, follow,
//! glow, parallax and the compare wipe, each over the three detailed sample pictures, fully loosened.
//! Also writes the mark atlas itself and the raw pictures.
//!
//! With `--material`, a material's sample sheet instead (`amenui::sample`): the sample page at each
//! looseness given, and with `--arrival` how the material arrives after the warm-up; the sheet checks
//! itself and the tool fails when it shows nothing it should. `--sweep <n>` draws it at n + 1 even steps
//! from 0 to 1 and prints how much of the title has changed from rest at each.
//!
//! ```text
//! usage: sheet <out_dir>
//!        sheet --material <name> [--loose 1,0.7,0.4,0.2,0 | --sweep <n>] [--theme <id>] [--arrival] [--out sheet.png]
//! ```
use amenui::pictures::{self, PICTURE_SIZES, SKY, TERRAIN, TREE};
use amenui::el::{El, col, picture, row};
use amenui::marks::{self, Atlas};
use amenui::offscreen::{Offscreen, save_png};
use amenui::style::{DARK, Look, Material, Markr, colour, set};
use amenui::ui::Ui;

const W: u32 = 1500;
const H: u32 = 620;

/// Two rows of the three pictures, fully loosened: row 1 uses slot 0, row 2 slot 1. With `wipe`,
/// nothing is loosened at rest and the tree of row 2 gets the compare line (the field holds one).
struct Sheet { wipe: Option<f32> }

fn view(s: &Sheet) -> El<()> {
    let line = |slot: u8, r: usize| {
        let mut out = row().gap(12.0);
        for id in [SKY, TERRAIN, TREE] {
            let (w, h) = PICTURE_SIZES[id as usize];
            let mut p = picture(id, h as f32 / w as f32).markr(slot).key(format!("p{r}/{id}")).grow(1.0);
            match s.wipe {
                Some(x) if r == 1 && id == TREE => p = p.wipr(x),
                Some(_) => {}
                None => p = p.loose(1.0),
            }
            out = out.child(p);
        }
        out
    };
    col().pad(12.0, 12.0).gap(12.0).child(line(0, 0)).child(line(1, 1))
}

/// `sheet --material …`: one material's sample sheet, checked.
fn material_sheet(args: &[String]) {
    let fail = |m: String| -> ! { eprintln!("sheet: {m}"); std::process::exit(2) };
    let (mut material, mut loose, mut theme, mut arrival, mut out) = (None, vec![1.0, 0.7, 0.4, 0.2, 0.0], 0, false, "sheet.png".to_owned());
    let mut sweep = None;
    let mut i = 0;
    while i < args.len() {
        let flag = args[i].as_str();
        if flag == "--arrival" { arrival = true; i += 1; continue; }
        let v = args.get(i + 1).cloned().unwrap_or_else(|| fail(format!("`{flag}` needs a value")));
        match flag {
            "--material" => material = Some(amenui::style::material_named(&v).unwrap_or_else(|e| fail(e.to_string()))),
            "--loose" => loose = v.split(',').map(|x| x.trim().parse::<f32>().unwrap_or_else(|_| fail(format!("`--loose {v}`: write loosenesses 0..1 with commas, e.g. 1,0.5,0")))).collect(),
            "--sweep" => sweep = Some(v.parse::<usize>().ok().filter(|n| (1..=16).contains(n)).unwrap_or_else(|| fail(format!("`--sweep {v}`: write how many steps from 0 to 1, 1–16")))),
            "--theme" => theme = amenui::style::theme_named(&v).unwrap_or_else(|e| fail(e.to_string())),
            "--out" => out = v,
            _ => fail(format!("unknown flag `{flag}`")),
        }
        i += 2;
    }
    let material = material.unwrap_or_else(|| fail("`--material` is required".into()));
    let faces = || amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).unwrap_or_else(|e| fail(e.to_string()));
    let mut o = Offscreen::try_new(amenui::sample::CELL.0, amenui::sample::CELL.1).unwrap_or_else(|e| fail(e.to_string()));
    let s = match sweep {
        Some(n) => amenui::sample::sweep(&mut o, &faces, material, n, theme),
        None => amenui::sample::sheet(&mut o, &faces, material, &loose, theme, arrival),
    }.unwrap_or_else(|e| { eprintln!("sheet: {e}"); std::process::exit(1) });
    if sweep.is_some() {
        for ((k, c), m) in s.cells.iter().zip(&s.changed).zip(&s.moved) {
            if let (Some(k), Some(c)) = (k, c) { println!("loose {k:.2}: {:.1} % of the title changed from rest, {:.1} % from the step before", c * 100.0, m.unwrap_or(0.0) * 100.0); }
        }
    }
    save_png(&out, s.w, s.h, &s.px);
    eprintln!("sheet: {} cells -> {out}", s.cells.len());
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a.starts_with("--")) { material_sheet(&args); return; }
    let out = std::env::args().nth(1).unwrap_or_else(|| "sheets".into());
    std::fs::create_dir_all(&out).expect("out dir");
    let mut o = Offscreen::new(W, H);
    let faces = amenui::text::load_bundled("assets").expect("fonts");
    let mut ui = Ui::new(Sheet { wipe: None }, view, |_, _| {}, faces, W, H, 1.0);
    ui.tick(100.0);

    // The atlas, enlarged 2×, mark in white and height in the second half.
    let atlas = Atlas::build(ui.text_mut(), amenui::text::MONO);
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
    for (id, w, h, px) in pictures::pictures() { save_png(&format!("{out}/picture-{id}.png"), w, h, &px); }

    let mut shoot = |ui: &mut Ui<Sheet, ()>, name: &str, a: Markr, b: Markr| {
        let mut look = Look { theme: DARK, material: Material::Plain, ..Look::default() };
        look.pics[0] = a; look.pics[1] = b;
        let px = o.render(ui, &look);
        save_png(&format!("{out}/{name}.png"), W, H, &px);
        eprintln!("{name}");
    };

    // Per set: the same set at two pitches, with values that suit it.
    for s in 0..set::COUNT as u8 {
        let base = match s {
            set::CHARS => Markr { pitch: 9.0, levels: 16.0, ground: 0.0, gamma: 1.1, ..Markr::with_set(s) },
            set::DITHER => Markr { pitch: 3.0, levels: 16.0, ground: 0.0, colour: colour::THEME_INK as f32, ..Markr::with_set(s) },
            set::PIXEL => Markr { pitch: 6.0, ..Markr::with_set(s) },
            set::MOSAIC | set::BRICK => Markr { pitch: 10.0, relief: 1.0, ground: 0.2, ..Markr::with_set(s) },
            set::GLOW => Markr { pitch: 8.0, levels: 12.0, ground: 0.0, glow: 1.0, gamma: 0.8, ..Markr::with_set(s) },
            set::STROKE => Markr { pitch: 9.0, levels: 8.0, ground: 0.15, follow: 1.0, ..Markr::with_set(s) },
            _ => Markr { pitch: 8.0, levels: 8.0, ground: 0.15, ..Markr::with_set(s) },
        };
        let big = Markr { pitch: base.pitch * 2.0, ..base };
        shoot(&mut ui, &format!("set-{s:02}-{}", ["chars", "dither", "pixel", "mosaic", "brick", "dots", "cross", "diagonal", "diamond", "lines", "glow", "stroke"][s as usize]), base, big);
    }
    // Palettes: blocky tone-quantised posters (solid pixels, dithered hand-over), and dither marks.
    for (i, (name, n, _)) in marks::PALETTES.iter().enumerate() {
        let poster = Markr { pitch: 6.0, set: set::PIXEL as f32, palette: i as f32, colours: *n as f32, spread: 0.5, ..Markr::default() };
        let dith = Markr { pitch: 3.0, set: set::DITHER as f32, levels: 16.0, palette: i as f32, colours: *n as f32, spread: 0.35, ground: 0.0, ..Markr::default() };
        shoot(&mut ui, &format!("palette-{i}-{n}"), poster, dith);
        eprintln!("  ({name})");
    }
    // Relief on and off, mosaic and brick.
    shoot(&mut ui, "relief-mosaic", Markr { pitch: 12.0, relief: 0.0, ..Markr::with_set(set::MOSAIC) }, Markr { pitch: 12.0, relief: 1.5, ..Markr::with_set(set::MOSAIC) });
    shoot(&mut ui, "relief-brick", Markr { pitch: 14.0, relief: 0.0, ..Markr::with_set(set::BRICK) }, Markr { pitch: 14.0, relief: 1.5, ..Markr::with_set(set::BRICK) });
    // Regions: by hash, then by brightness band.
    let reg = Markr { pitch: 7.0, levels: 12.0, ground: 0.05, regions: 6.0, region_count: 4.0, ..Markr::default() };
    shoot(&mut ui, "regions", Markr { region_by: 0.0, ..reg }, Markr { region_by: 1.0, ..reg });
    // Follow: strokes unturned, then along the contours.
    let st = Markr { pitch: 10.0, levels: 8.0, ground: 0.1, ..Markr::with_set(set::STROKE) };
    shoot(&mut ui, "follow", Markr { follow: 0.0, ..st }, Markr { follow: 1.0, ..st });
    // Colour modes on characters: theme ink, then two colours by brightness.
    let ch = Markr { pitch: 16.0, levels: 16.0, ground: 0.0, gamma: 0.6, ..Markr::with_set(set::CHARS) };
    shoot(&mut ui, "colour-modes", Markr { colour: colour::THEME_INK as f32, ..ch }, Markr { colour: colour::TWO as f32, tint_a: [0.1, 0.3, 0.9], tint_b: [1.0, 0.85, 0.3], ..ch });
    // Glow off and on.
    let gl = Markr { pitch: 8.0, levels: 12.0, ground: 0.0, gamma: 0.8, ..Markr::with_set(set::GLOW) };
    shoot(&mut ui, "glow", Markr { glow: 0.0, ..gl }, Markr { glow: 1.5, ..gl });
    // Ordered spread between levels: hard, then dithered.
    let sp = Markr { pitch: 6.0, levels: 4.0, ground: 0.0, ..Markr::with_set(set::DOTS) };
    shoot(&mut ui, "spread", Markr { spread: 0.0, ..sp }, Markr { spread: 1.0, ..sp });
    // Parallax toward a pointer in the middle of the sheet.
    ui.pointer_move(W as f32 * 0.5, H as f32 * 0.5);
    shoot(&mut ui, "depth", Markr { depth: 0.0, ..ch }, Markr { depth: 12.0, ..ch });
    ui.pointer_leave();
    // The compare wipe, left original, right loosened.
    ui.state.wipe = Some(0.5);
    ui.relayout();
    shoot(&mut ui, "wipe", ch, Markr { pitch: 5.0, ..Markr::with_set(set::DITHER) });
}
