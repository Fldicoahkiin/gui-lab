//! The command line: `amenui add <component>… [--to <dir>] [--force]` writes components' source (and what
//! they depend on, compiled into the binary) into a project, as `registry.json` lists them;
//! `amenui add material <name>…` writes materials' source (`src/materials/<name>.wgsl`).
//!
//! usage: amenui add [material] <name>… [--to <dir>] [--force]
fn main() -> std::process::ExitCode {
    const USAGE: &str = "usage: amenui add [material] <name>… [--to <dir>] [--force]";
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("add") { eprintln!("{USAGE}"); return std::process::ExitCode::from(2); }
    let (mut names, mut to, mut force) = (vec![], std::path::PathBuf::from("."), false);
    while let Some(a) = args.next() {
        match a.as_str() { "--to" => to = args.next().map(Into::into).unwrap_or(to), "--force" => force = true, _ => names.push(a) }
    }
    let material = names.first().is_some_and(|n| n == "material");
    let names: Vec<&str> = names.iter().skip(usize::from(material)).map(String::as_str).collect();
    if names.is_empty() { eprintln!("{USAGE}"); return std::process::ExitCode::from(2); }
    let files = if material { amenui::add::material_files(amenui::sources::REGISTRY, &names) } else { amenui::add::files(amenui::sources::REGISTRY, &names) };
    match files.and_then(|f| amenui::add::write(&to, &f, force)) {
        Ok(w) => { for p in w { println!("wrote {}", p.display()); } std::process::ExitCode::SUCCESS }
        Err(e) => { eprintln!("{e}"); std::process::ExitCode::FAILURE }
    }
}
