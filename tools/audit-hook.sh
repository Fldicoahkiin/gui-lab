#!/bin/sh
# Claude Code PostToolUse hook (.claude/settings.json): after an Edit or Write to a view file, render the
# one page it belongs to and audit it (`render --audit --strict`). A recipe (examples/<name>.rs) is
# `recipe-<name>`; a component's examples (examples/components/<name>.rs) and its widget
# (src/widgets/<name>.rs) are `component-<name>`. Anything else returns at once. With a P0 or P1
# finding, or a problem in the view, the findings go back to the model (exit 2, on stderr).
root="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/.." && pwd)}"
file=$(tr -d '\n' | sed -n 's/.*"file_path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
[ -n "$file" ] || exit 0
rel=${file#"$root"/}
name=$(basename "$rel" .rs)
case "$rel" in
  examples/components/*.rs) page="component-$name" ;;
  examples/*.rs) page="recipe-$name" ;;
  src/widgets/*.rs) [ -f "$root/examples/components/$name.rs" ] || exit 0; page="component-$name" ;;
  *) exit 0 ;;
esac
out=$(cd "$root" && cargo run --release --quiet --bin render -- --page "$page" --size 1280x800 --audit --strict --out "${TMPDIR:-/tmp}/amenui-audit-hook.png" 2>&1) && exit 0
found=$(printf '%s\n' "$out" | grep -E '^(P0|P1) |^render: ')
[ -n "$found" ] || found=$(printf '%s\n' "$out" | tail -n 30)
printf 'render --page %s --audit --strict, after the change to %s:\n%s\n' "$page" "$rel" "$found" >&2
exit 2
