#!/bin/sh
# Writes .claude/skills/amenui/SKILL.md: the skill's front matter, then AGENTS.md as it is.
# tests/skill.rs checks the file is exactly this, so the skill never says other than AGENTS.md.
root=$(cd "$(dirname "$0")/.." && pwd)
{
  cat <<'HEAD'
---
name: amenui
description: Writing interfaces with amenui, the self-drawn Rust and wgpu framework in this repository. Use when writing or changing a view, a widget, a recipe or a component example (examples/, src/widgets/), and to verify one with render --audit --strict.
---

<!-- Made by tools/skill.sh from AGENTS.md; do not edit here. -->

HEAD
  cat "$root/AGENTS.md"
} > "$root/.claude/skills/amenui/SKILL.md"
