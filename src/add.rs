//! `lab add`: copy a component's source into a project, shadcn-style (DESIGN.md §3). The registry
//! (`registry.json`) names each component's files and the components it depends on; `add` resolves
//! the dependencies, then copies every file, keeping its path under the destination (so
//! `src/widgets/button.rs` lands at `<to>/src/widgets/button.rs`), and says what it wrote. A file that
//! is already there is left alone unless `overwrite` is set.
use crate::error::AddError;
use std::path::{Path, PathBuf};

/// The files of `names` and everything they depend on, in dependency order, with no repeats.
pub fn files(registry: &str, names: &[&str]) -> Result<Vec<String>, AddError> {
    let reg: serde_json::Value = serde_json::from_str(registry).map_err(|e| AddError::Registry(e.to_string()))?;
    let items = reg["items"].as_array().ok_or_else(|| AddError::Registry("no `items` list".into()))?;
    let known: Vec<&str> = items.iter().filter_map(|i| i["name"].as_str()).collect();
    let mut out: Vec<String> = vec![];
    let mut seen: Vec<String> = vec![];
    fn visit(n: &str, items: &[serde_json::Value], known: &[&str], seen: &mut Vec<String>, out: &mut Vec<String>) -> Result<(), AddError> {
        if seen.iter().any(|s| s == n) { return Ok(()); }
        seen.push(n.to_owned());
        let item = items.iter().find(|i| i["name"].as_str() == Some(n) || i["aliases"].as_array().is_some_and(|a| a.iter().any(|x| x.as_str() == Some(n))))
            .ok_or_else(|| AddError::Unknown { name: n.to_owned(), near: near(n, known), known: known.join(", ") })?;
        for d in item["dependencies"].as_array().into_iter().flatten().filter_map(|d| d.as_str()) { visit(d, items, known, seen, out)?; }
        for f in item["files"].as_array().into_iter().flatten().filter_map(|f| f.as_str()) { if !out.iter().any(|o| o == f) { out.push(f.to_owned()); } }
        Ok(())
    }
    for n in names { visit(n, items, &known, &mut seen, &mut out)?; }
    // The framework files every component needs.
    for r in reg["requires"].as_array().into_iter().flatten().filter_map(|r| r.as_str()) { if !out.iter().any(|o| o == r) { out.insert(0, r.to_owned()); } }
    Ok(out)
}
/// The known name closest to `n` (for the error).
fn near(n: &str, known: &[&str]) -> String {
    let d = |a: &str, b: &str| { let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect()); let mut p: Vec<usize> = (0..=b.len()).collect();
        for i in 1..=a.len() { let mut c = vec![i; b.len() + 1]; for j in 1..=b.len() { c[j] = (p[j] + 1).min(c[j - 1] + 1).min(p[j - 1] + usize::from(a[i - 1] != b[j - 1])); } p = c; } p[b.len()] };
    known.iter().min_by_key(|k| d(n, k)).map(|s| s.to_string()).unwrap_or_default()
}
/// Copy `files` from the framework's root `from` to `to`; returns the files written.
pub fn copy(from: &Path, to: &Path, files: &[String], overwrite: bool) -> Result<Vec<PathBuf>, AddError> {
    let mut wrote = vec![];
    for f in files {
        let (src, dst) = (from.join(f), to.join(f));
        if dst.exists() && !overwrite { continue; }
        if let Some(p) = dst.parent() { std::fs::create_dir_all(p).map_err(|e| AddError::Io { path: p.display().to_string(), msg: e.to_string() })?; }
        std::fs::copy(&src, &dst).map_err(|e| AddError::Io { path: src.display().to_string(), msg: e.to_string() })?;
        wrote.push(dst);
    }
    Ok(wrote)
}
