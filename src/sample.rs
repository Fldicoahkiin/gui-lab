//! Sample sheets of a material that check themselves (DESIGN.md §1.6): one small page drawn at several
//! loosenesses side by side, and how the material arrives after the warm-up (plain, then 120 ms and
//! 300 ms after the material is ready). A sheet asserts what it should show, so a sheet that comes out
//! empty or the same in every cell fails instead of being looked at: neighbouring cells differ, the
//! fully loose cell of `scribblr` has a thread, and the cell at rest is plain's frame, byte for byte, for
//! a material whose rest is plain. `sheet --material <name> --loose 1,0.7,0.4,0.2,0` writes one.
#![warn(missing_docs)]
use crate::el::{El, Font, Paint, Response, col, picture, row, text, text_lh};
use crate::offscreen::Offscreen;
use crate::style::{Look, Material, THEMES};
use crate::ui::Ui;

/// One cell's size, device px.
pub const CELL: (u32, u32) = (520, 300);

/// What went wrong with a sheet: what it asserts did not hold.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SampleError {
    /// Two neighbouring cells are the same picture.
    #[error("`{material}`: the cells at looseness {a} and {b} are the same picture: the sheet shows nothing between them (is the looseness reaching the page?)")]
    Same {
        /// The material.
        material: String,
        /// The first cell's looseness.
        a: f32,
        /// The second's.
        b: f32,
    },
    /// The fully loose cell of `scribblr` drew no thread.
    #[error("`{material}`: the cell at looseness {k} drew no thread: the title should come apart into one line")]
    NoThread {
        /// The material.
        material: String,
        /// The cell's looseness.
        k: f32,
    },
    /// The cell at rest is not plain's frame.
    #[error("`{material}`: the cell at rest differs from plain in {pixels} px; at rest this material is plain, byte for byte")]
    NotPlain {
        /// The material.
        material: String,
        /// How many pixels differ.
        pixels: usize,
    },
    /// The arrival frame is the plain frame.
    #[error("`{material}`: 120 ms after it is ready, the material is not on the page (the frame is plain's)")]
    NotArrived {
        /// The material.
        material: String,
    },
}

/// The sample page's state: how loose its decoration is.
pub struct Sample {
    /// The looseness set on the title, the rule and the picture (text to read holds as it always does).
    pub k: f32,
    /// The words on top (the material and the looseness, or the stage of the arrival).
    pub label: String,
    /// The look it is drawn in (the runtime reads its timings and material from it).
    pub look: Look,
    /// Commands for the runtime (a stir through the title, so the field map has energy where knots form).
    pub cmds: Vec<crate::morph::Cmd>,
}

/// The sample page: a label, a large title and a rule loosened by `k`, a picture beside the title, a
/// paragraph to read and a solid button.
pub fn view(s: &Sample) -> El<()> {
    col().key("sample").pad(20.0, 16.0).gap(12.0)
        .child(text(s.label.clone(), 11.0, Paint::DIM).font(Font::Mono).key("label"))
        .child(row().gap(18.0).align(taffy::AlignItems::END)
            .child(text("loose thread", 40.0, Paint::INK).font(Font::Display).key("title").legible(false).respond(Response::Loosen).loose(s.k))
            .child(col().width(120.0).child(picture(crate::pictures::SKY, 0.62).key("picture").loose(s.k))))
        .child(crate::el::space(160.0, 4.0).fill(Paint::ACCENT).key("rule").loose(s.k))
        .child(row().gap(16.0).align(taffy::AlignItems::CENTER)
            .child(text_lh("At rest the page is only type and a picture; loosened, its title comes apart.", 14.0, 20.0, Paint::TEXT).width(300.0).key("body"))
            .child(crate::widgets::button_as("send", "Send", crate::widgets::Variant::Accent, ())))
}

/// A sheet: its size and RGBA pixels (cells left to right, then down), and the looseness of each cell
/// (`None` for the arrival cells).
pub struct Sheet {
    /// Width, device px.
    pub w: u32,
    /// Height, device px.
    pub h: u32,
    /// RGBA8.
    pub px: Vec<u8>,
    /// Each cell's looseness, in order; the arrival cells have `None`.
    pub cells: Vec<Option<f32>>,
}

/// Whether `material` draws its rest as plain does, byte for byte (`scribblr`; the others have a look of
/// their own at rest: a ground, a grain, bands).
pub fn rests_as_plain(material: Material) -> bool { matches!(material, Material::Plain | Material::Scribblr) }

fn page(faces: Vec<crate::text::Face>, k: f32, label: String, look: Look) -> Ui<Sample, ()> {
    // Loose cells stir the field map through the title, as a pointer passing would: the knots form where it
    // has energy.
    let cmds = if k > 0.0 { vec![crate::morph::Cmd::Stir { at: crate::morph::StirAt::Key("title".into()), v: [1.0, 0.15], energy: 1.0, radius: 200.0, scope: crate::el::Scope::Global }] } else { vec![] };
    let mut ui = Ui::new(Sample { k, label, look, cmds }, view, |_, _| {}, faces, CELL.0, CELL.1, 1.0).on_look(|s| s.look).on_commands(|s| std::mem::take(&mut s.cmds));
    ui.relayout();
    ui
}

/// Draw `material`'s sheet (`faces` gives the fonts for each cell's page) at the loosenesses `loose` in `theme` (a place in `style::THEMES`), and with
/// `arrival`, three more cells: plain, then 120 ms and 300 ms after the material is ready (each element
/// condensing from its cap, as after the warm-up). Checks what the sheet should show (`SampleError`).
pub fn sheet(o: &mut Offscreen, faces: &dyn Fn() -> Vec<crate::text::Face>, material: Material, loose: &[f32], theme: usize, arrival: bool) -> Result<Sheet, SampleError> {
    let name = material.name().to_owned();
    let look = Look { material, theme: THEMES[theme.min(THEMES.len() - 1)], ..Look::default() };
    let plain = crate::render::plain(&look);
    let mut frames: Vec<(u32, u32, Vec<u8>)> = vec![];
    let mut cells = vec![];
    for &k in loose {
        let mut ui = page(faces(), k, format!("{name} · loose {k:.2}"), look);
        // A while after the stir, frame by frame: what is held loose is held, the map still warm.
        let mut t = 0.0;
        while t < 0.5 { t += 1.0 / 60.0; ui.tick(t); }
        let px = o.render(&mut ui, &look);
        if k >= 1.0 && material == Material::Scribblr && ui.strands().iter().all(|s| s.2.is_empty()) { return Err(SampleError::NoThread { material: name, k }); }
        if k <= 0.0 && rests_as_plain(material) {
            let mut p = page(faces(), k, format!("{name} · loose {k:.2}"), plain);
            let mut t = 0.0;
            while t < 0.5 { t += 1.0 / 60.0; p.tick(t); }
            let ppx = o.render(&mut p, &plain);
            let pixels = px.chunks(4).zip(ppx.chunks(4)).filter(|(a, b)| a != b).count();
            if pixels > 0 { return Err(SampleError::NotPlain { material: name, pixels }); }
        }
        if let (Some((_, _, prev)), Some(Some(pk))) = (frames.last(), cells.last()) && *prev == px { return Err(SampleError::Same { material: name, a: *pk, b: k }); }
        frames.push((CELL.0, CELL.1, px));
        cells.push(Some(k));
    }
    if arrival {
        // Plain until ready; then everything condenses into the material from its cap.
        let mut ui = page(faces(), 0.0, format!("{name} · plain, before it is ready"), look);
        ui.tick(3.0);
        frames.push((CELL.0, CELL.1, o.render(&mut ui, &plain)));
        let plain_px = frames.last().map(|f| f.2.clone()).unwrap_or_default();
        cells.push(None);
        // The label says the first stage before the material arrives, so its own change of text has
        // settled by the first frame.
        ui.state.label = format!("{name} · ready +120 ms");
        ui.relayout();
        ui.tick(3.5);
        let ready: f32 = 3.5;
        ui.condense_all();
        for (ms, stage) in [(120.0, "+120 ms"), (300.0, "+300 ms")] {
            // Frame by frame at 60 Hz up to the moment, as a host would.
            let end = ready + ms / 1000.0;
            let label = format!("{name} · ready {stage}");
            if ui.state.label != label { ui.state.label = label; ui.relayout(); }
            let mut t = ui.now();
            while t < end - 1e-6 { t = (t + 1.0 / 60.0).min(end); ui.tick(t); o.render(&mut ui, &look); }
            let px = o.render(&mut ui, &look);
            // Below the label (which says the stage), the page itself.
            let below = (34 * CELL.0 * 4) as usize;
            if ms == 120.0 && !rests_as_plain(material) && px[below..] == plain_px[below..] { return Err(SampleError::NotArrived { material: name }); }
            frames.push((CELL.0, CELL.1, px));
            cells.push(None);
        }
    }
    let cols = frames.len().min(if arrival { 4 } else { 5 });
    let (w, h, px) = crate::offscreen::sheet(&frames, cols);
    Ok(Sheet { w, h, px, cells })
}
