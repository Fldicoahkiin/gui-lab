//! Sample sheets check themselves (`amenui::sample`): every material's sheet at 1, 0.5 and 0 with its
//! arrival cells holds what it asserts; `scribblr`'s at 1, 0.7, 0.4, 0.2 and 0 has a thread when fully
//! loose and is plain at rest; a sheet whose cells are all the same fails. Needs a GPU adapter
//! (lavapipe will do); without one the tests say so and pass.
use amenui::offscreen::Offscreen;
use amenui::sample::{CELL, SampleError, sheet};
use amenui::style::{MATERIALS, Material};

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

#[test]
fn every_material_s_sheet_holds_what_it_asserts() {
    let Ok(mut o) = Offscreen::try_new(CELL.0, CELL.1) else { eprintln!("no GPU adapter here: skipped"); return };
    let s = sheet(&mut o, &fonts, Material::Scribblr, &[1.0, 0.7, 0.4, 0.2, 0.0], 0, true).expect("scribblr's sheet");
    assert_eq!(s.cells.len(), 8);
    assert_eq!(s.px.len(), (s.w * s.h * 4) as usize);
    for (m, name) in MATERIALS {
        for theme in [0, 1] {
            if let Err(e) = sheet(&mut o, &fonts, m, &[1.0, 0.5, 0.0], theme, true) { panic!("{name}, theme {theme}: {e}"); }
        }
    }
}

#[test]
fn a_sheet_that_shows_nothing_fails() {
    let Ok(mut o) = Offscreen::try_new(CELL.0, CELL.1) else { eprintln!("no GPU adapter here: skipped"); return };
    let e = sheet(&mut o, &fonts, Material::Scribblr, &[0.0, 0.0], 0, false).err();
    assert!(matches!(e, Some(SampleError::Same { .. })), "{e:?}");
}
