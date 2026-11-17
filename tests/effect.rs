//! Putting a material, a field and a scope on text that already exists (DESIGN.md §1.6): `el::wrap_text`
//! makes a page to read from plain text or Markdown, and `El::effect` puts the effect on it in one line.
use amenui::el::{Effect, El, Response, Scope, wrap_text};
use amenui::style::Material;

const DOC: &str = "# A page\n\nSome text that was already written, with a [link](#elsewhere).\n\n- one\n- two\n";

fn view(_: &()) -> El<()> {
    wrap_text::<()>(DOC).effect(Effect { material: Some(Material::Print), response: Some(Response::Loosen), scope: Some(Scope::Container(None)) })
}

#[test]
fn text_is_wrapped_into_a_page_and_takes_the_effect_whole() {
    let mut ui = amenui::ui::Ui::new((), view, |_, _| {}, amenui::text::embedded(), 720, 480, 1.0);
    ui.tick(1.0);
    let tree = ui.export_tree();
    let root = tree.find("wrap").expect("the page is keyed wrap");
    assert_eq!(root.material, "print");
    assert_eq!(root.response, "loosen");
    let heading = tree.find("wrap/h0").expect("the heading");
    assert_eq!(heading.text.as_deref(), Some("A page"));
    assert_eq!(heading.material, "print", "the material is the subtree's");
    let para = tree.find("wrap/b1/text").expect("the paragraph");
    assert_eq!(para.text.as_deref(), Some("Some text that was already written, with a link."));
    assert!(para.legible, "text to read stays legible under the effect");
    assert!(tree.find("wrap/b1/link0").is_none(), "a link shows as its text only");
    // What is not said stays as it was.
    let none: El<()> = wrap_text::<()>(DOC).effect(Effect::default());
    assert_eq!(none.material, None);
}
