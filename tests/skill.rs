//! The Claude Code skill and save hook in the repository (.claude/): the skill is AGENTS.md as it is, under
//! its front matter (made by tools/skill.sh), so it cannot say other than AGENTS.md; the hook is set up
//! after Edit and Write and renders only the page a view file belongs to; every component has a page.
use std::process::{Command, Stdio};
use std::io::Write;

fn root() -> String { env!("CARGO_MANIFEST_DIR").to_owned() }
fn read(p: &str) -> String { std::fs::read_to_string(format!("{}/{p}", root())).unwrap_or_else(|e| panic!("{p}: {e}")) }

#[test]
fn the_skill_is_agents_md_under_its_front_matter() {
    let skill = read(".claude/skills/amenui/SKILL.md");
    let agents = read("AGENTS.md");
    assert!(skill.starts_with("---\nname: amenui\ndescription: "), "the front matter names the skill and says when to use it");
    let head = skill.find("\n---\n").expect("the front matter ends") + 5;
    let body = skill[head..].split_once("-->\n\n").map(|(_, b)| b).expect("the note that says where it comes from");
    assert!(body == agents, "SKILL.md differs from AGENTS.md: run tools/skill.sh");
}

#[test]
fn the_save_hook_runs_the_audit_after_an_edit_or_a_write() {
    let s: serde_json::Value = serde_json::from_str(&read(".claude/settings.json")).expect("settings.json is JSON");
    let post = &s["hooks"]["PostToolUse"][0];
    let m = post["matcher"].as_str().expect("a matcher");
    assert!(m.split('|').any(|t| t == "Edit") && m.split('|').any(|t| t == "Write"), "{m}");
    let cmd = post["hooks"][0]["command"].as_str().expect("a command");
    assert!(cmd.contains("tools/audit-hook.sh"), "{cmd}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for t in ["tools/audit-hook.sh", "tools/skill.sh"] {
            let mode = std::fs::metadata(format!("{}/{t}", root())).expect(t).permissions().mode();
            assert!(mode & 0o111 != 0, "{t} is executable");
        }
        // A file that is not a view returns at once and says nothing (no render).
        for f in ["README.md", "src/ui.rs", "src/widgets/mod.rs"] {
            let mut c = Command::new(format!("{}/tools/audit-hook.sh", root())).env("CLAUDE_PROJECT_DIR", root()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("the hook runs");
            write!(c.stdin.take().expect("stdin"), "{{\"tool_name\":\"Edit\",\"tool_input\":{{\"file_path\":\"{}/{f}\"}}}}", root()).expect("input");
            let o = c.wait_with_output().expect("it ends");
            assert!(o.status.success() && o.stdout.is_empty() && o.stderr.is_empty(), "{f}: {o:?}");
        }
    }
}

#[test]
fn every_component_has_a_page_that_lays_out_with_no_problems() {
    let fonts = || amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts");
    for (name, _) in amenui::recipes::components::all() {
        let mut d = amenui::drive::open(&format!("component-{name}"), fonts(), 1280, 800, 1.0, None, None).unwrap_or_else(|e| panic!("{name}: {e}"));
        amenui::drive::play(d.as_mut(), &[], 300.0).expect("play");
        assert!(d.tree().find("ex/0").is_some(), "{name}: its first example is on the page");
        assert!(d.problems().is_empty(), "{name}: {:?}", d.problems());
    }
    assert!(amenui::drive::open("component-nothing", fonts(), 100, 100, 1.0, None, None).is_err());
}
