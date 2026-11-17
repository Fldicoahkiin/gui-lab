//! The command line: `amenui add <component>… [--to <dir>] [--force]` copies components' source (and what
//! they depend on) from this repository into a project, as `registry.json` lists them.
//!
//! usage: amenui add <name>… [--to <dir>] [--force]
fn main() -> std::process::ExitCode {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("add") { eprintln!("usage: amenui add <name>… [--to <dir>] [--force]"); return std::process::ExitCode::from(2); }
    let (mut names, mut to, mut force) = (vec![], std::path::PathBuf::from("."), false);
    while let Some(a) = args.next() {
        match a.as_str() { "--to" => to = args.next().map(Into::into).unwrap_or(to), "--force" => force = true, _ => names.push(a) }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let reg = std::fs::read_to_string(root.join("registry.json")).unwrap_or_default();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    match amenui::add::files(&reg, &names).and_then(|f| amenui::add::copy(root, &to, &f, force)) {
        Ok(w) => { for p in w { println!("wrote {}", p.display()); } std::process::ExitCode::SUCCESS }
        Err(e) => { eprintln!("{e}"); std::process::ExitCode::FAILURE }
    }
}
