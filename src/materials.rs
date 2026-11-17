//! Materials as source (DESIGN.md §1.6): each material is one WGSL file, `src/materials/<name>.wgsl`,
//! and the material pass's shader is the framework's own (`shader.wgsl`) with every material's file
//! put in at its place, in id order. A file begins with its name and its numbers:
//!
//! ```text
//! // @material smudgr
//! // @param 纸纹 0.0 1.0 0.05 2 - 0.6
//! ```
//!
//! (`@param <label> <min> <max> <step> <decimals> <unit, or -> <default>`, in the order of
//! `style::MaterialParams`), then its function `fn m_<name>(p: vec2f) -> vec3f`, which reads its own
//! numbers as `mp(SELF, 0)` and `mp(SELF, 1)` (two `vec4f`s). `amenui add material <name>` copies a
//! file into a project.
#![warn(missing_docs)]

/// The framework's shader without the materials.
const CORE: &str = include_str!("shader.wgsl");
/// Where the materials go: after the comment that names them.
const PLACE: &str = "src/materials/<name>.wgsl, in id order";

/// Every built-in material's source, in id order, as `(name, wgsl)`.
pub const BUILT_IN: [(&str, &str); crate::style::MATERIAL_COUNT] = [
    ("plain", include_str!("materials/plain.wgsl")),
    ("dottr", include_str!("materials/dottr.wgsl")),
    ("halftonr", include_str!("materials/halftonr.wgsl")),
    ("charr", include_str!("materials/charr.wgsl")),
    ("smudgr", include_str!("materials/smudgr.wgsl")),
    ("carvr", include_str!("materials/carvr.wgsl")),
    ("glitchr", include_str!("materials/glitchr.wgsl")),
    ("near", include_str!("materials/near.wgsl")),
    ("scribblr", include_str!("materials/scribblr.wgsl")),
];

/// The source of the built-in material `name` (`src/materials/<name>.wgsl`).
pub fn source(name: &str) -> Option<&'static str> { BUILT_IN.iter().find(|m| m.0 == name).map(|m| m.1) }

/// One number of a material, as its file's header gives it.
#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    /// Its label.
    pub name: String,
    /// The smallest value.
    pub min: f32,
    /// The largest value.
    pub max: f32,
    /// How far one step of its control moves it.
    pub step: f32,
    /// Digits after the point when it is shown.
    pub decimals: usize,
    /// Its unit, or empty.
    pub unit: String,
    /// The value it starts at.
    pub default: f32,
}

/// What a material file's header says: its name and its numbers.
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    /// The name after `@material`.
    pub name: String,
    /// The numbers, in order (at most eight).
    pub params: Vec<Param>,
}

/// Read the header of a material file.
pub fn header(wgsl: &str) -> Result<Header, crate::error::MaterialError> {
    use crate::error::MaterialError as E;
    let mut lines = wgsl.lines();
    let name = lines.next().and_then(|l| l.strip_prefix("// @material ")).map(str::trim).filter(|n| !n.is_empty())
        .ok_or(E::NoName)?.to_owned();
    let mut params = vec![];
    for (i, l) in lines.map_while(|l| l.strip_prefix("// @param ")).enumerate() {
        let f: Vec<&str> = l.split_whitespace().collect();
        let bad = || E::Param { line: i + 2, text: l.to_owned() };
        let [label, min, max, step, decimals, unit, default] = f[..] else { return Err(bad()) };
        let n = |s: &str| s.parse::<f32>().map_err(|_| bad());
        params.push(Param { name: label.to_owned(), min: n(min)?, max: n(max)?, step: n(step)?, decimals: decimals.parse().map_err(|_| bad())?,
            unit: if unit == "-" { String::new() } else { unit.to_owned() }, default: n(default)? });
    }
    if params.len() > 8 { return Err(E::TooMany { count: params.len() }); }
    Ok(Header { name, params })
}

/// A built-in material's code as it goes into the shader: `SELF` is its own id's constant.
fn built_in(name: &str, wgsl: &str) -> String { replace_word(wgsl, "SELF", &name.to_uppercase()) }

/// `text` with every whole-word `from` replaced by `to`.
pub(crate) fn replace_word(text: &str, from: &str, to: &str) -> String {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(from) {
        let before = rest[..i].chars().next_back();
        let after = rest[i + from.len()..].chars().next();
        out.push_str(&rest[..i]);
        out.push_str(if before.is_some_and(word) || after.is_some_and(word) { from } else { to });
        rest = &rest[i + from.len()..];
    }
    out.push_str(rest);
    out
}

/// The most materials there can be, built-in and added (an id is four bits of an instance's code).
pub const MAX: usize = 16;

/// A material added from source (`Look::material_source`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Added {
    /// The name it was added under.
    pub name: &'static str,
    /// Its id (9–15).
    pub id: u8,
    /// The numbers it is drawn with.
    pub params: crate::style::MaterialParams,
}

/// Every added material, and its code as it goes into the shader.
static ADDED: std::sync::RwLock<Vec<(Added, String)>> = std::sync::RwLock::new(Vec::new());

fn added_list() -> std::sync::RwLockReadGuard<'static, Vec<(Added, String)>> { ADDED.read().unwrap_or_else(|e| e.into_inner()) }

/// The material added with id `id`.
pub fn added(id: u8) -> Option<Added> { added_list().iter().find(|a| a.0.id == id).map(|a| a.0) }
/// The id of the material added under `name`.
pub fn added_named(name: &str) -> Option<u8> { added_list().iter().find(|a| a.0.name == name).map(|a| a.0.id) }

/// Add the material in `wgsl` under `name` (`Look::material_source`): its functions are renamed for
/// `name` (`m_<its name>` becomes `m_<name>`, any other `f` becomes `f_<name>`) and `SELF` becomes
/// its id. Returns the id.
pub fn add(name: &str, wgsl: &str, params: crate::style::MaterialParams) -> Result<u8, crate::error::MaterialError> {
    use crate::error::MaterialError as E;
    let ok = name.starts_with(|c: char| c.is_ascii_lowercase()) && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !ok { return Err(E::BadName { name: name.to_owned() }); }
    if BUILT_IN.iter().any(|m| m.0 == name) { return Err(E::BuiltIn { name: name.to_owned() }); }
    let h = header(wgsl)?;
    let main = format!("m_{}", h.name);
    if !wgsl.contains(&format!("fn {main}(")) { return Err(E::NoFunction { name: h.name }); }
    let mut code = wgsl.to_owned();
    for f in wgsl.lines().filter_map(|l| l.strip_prefix("fn ")).filter_map(|l| l.split('(').next()).map(str::trim) {
        let to = if f == main { format!("m_{name}") } else { format!("{f}_{name}") };
        code = replace_word(&code, f, &to);
    }
    let mut list = ADDED.write().unwrap_or_else(|e| e.into_inner());
    let id = match list.iter().position(|a| a.0.name == name) {
        Some(i) => list[i].0.id,
        None if crate::style::MATERIAL_COUNT + list.len() < MAX => (crate::style::MATERIAL_COUNT + list.len()) as u8,
        None => return Err(E::Full { max: MAX - crate::style::MATERIAL_COUNT }),
    };
    let code = replace_word(&code, "SELF", &id.to_string());
    let a = Added { name: list.iter().find(|a| a.0.name == name).map_or_else(|| &*Box::leak(name.to_owned().into_boxed_str()), |a| a.0.name), id, params };
    match list.iter_mut().find(|a| a.0.id == id) { Some(e) => *e = (a, code), None => list.push((a, code)) }
    Ok(id)
}

/// The material pass's shader: the framework's own with every material put in at its place, built-in
/// ones in id order and then those added, each added one dispatched on its id.
pub fn shader() -> String {
    let at = CORE.find(PLACE).and_then(|i| CORE[i..].find("\n\n").map(|j| i + j + 2)).expect("shader.wgsl names the materials' place");
    let mut s = String::with_capacity(CORE.len() + 16 * 1024);
    s.push_str(&CORE[..at]);
    for (name, wgsl) in BUILT_IN { s.push_str(&built_in(name, wgsl)); s.push('\n'); }
    let list = added_list();
    for (_, code) in list.iter() { s.push_str(code); s.push('\n'); }
    const DEFAULT: &str = "      default: { col = m_plain(p); }";
    let (before, after) = CORE[at..].split_once(DEFAULT).expect("shader.wgsl dispatches on the material");
    s.push_str(before);
    for (a, _) in list.iter() { s.push_str(&format!("      case {}: {{ col = m_{}(p); }}\n", a.id, a.name)); }
    s.push_str(DEFAULT);
    s.push_str(after);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_file_heads_with_its_name_and_the_numbers_the_framework_gives_it() {
        for (i, (name, wgsl)) in BUILT_IN.iter().enumerate() {
            let h = header(wgsl).unwrap();
            assert_eq!(h.name, *name);
            assert_eq!(crate::style::MATERIALS[i].1, *name, "the files are in id order");
            let ps = crate::style::MAT_PARAMS[i];
            assert_eq!(h.params.len(), ps.len(), "{name}");
            for (a, b) in h.params.iter().zip(ps) {
                assert_eq!((a.name.as_str(), a.min, a.max, a.step, a.decimals, a.unit.as_str(), a.default), (b.name, b.min, b.max, b.step, b.decimals, b.unit, b.default), "{name}");
            }
            assert!(wgsl.contains(&format!("fn m_{name}(")), "{name} has its function");
        }
    }

    #[test]
    fn a_header_that_does_not_say_its_name_or_its_numbers_is_refused() {
        assert_eq!(header("fn m_x(p: vec2f) -> vec3f { return vec3f(0.0); }"), Err(crate::error::MaterialError::NoName));
        assert!(matches!(header("// @material x\n// @param a 0 1 0.1\n"), Err(crate::error::MaterialError::Param { line: 2, .. })));
        assert_eq!(replace_word("mp(SELF, 0) SELFISH xSELF", "SELF", "PRINT"), "mp(PRINT, 0) SELFISH xSELF");
    }
}
