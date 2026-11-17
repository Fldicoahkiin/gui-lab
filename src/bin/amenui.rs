//! The command line: `amenui add <component>… [--to <dir>] [--force]` writes components' source (and what
//! they depend on, compiled into the binary) into a project, as `registry.json` lists them.
//!
//! usage: amenui add <name>… [--to <dir>] [--force]
fn main() -> std::process::ExitCode {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("add") { eprintln!("usage: amenui add <name>… [--to <dir>] [--force]"); return std::process::ExitCode::from(2); }
    let (mut names, mut to, mut force) = (vec![], std::path::PathBuf::from("."), false);
    while let Some(a) = args.next() {
        match a.as_str() { "--to" => to = args.next().map(Into::into).unwrap_or(to), "--force" => force = true, _ => names.push(a) }
    }
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    match amenui::add::files(amenui::sources::REGISTRY, &names).and_then(|f| amenui::add::write(&to, &f, force)) {
        Ok(w) => { for p in w { println!("wrote {}", p.display()); } std::process::ExitCode::SUCCESS }
        Err(e) => { eprintln!("{e}"); std::process::ExitCode::FAILURE }
    }
}
