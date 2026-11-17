# Documentation

Start with [AGENTS.md](../AGENTS.md) to write an interface, then the [recipes](recipes/) for complete
small applications. The [design](DESIGN.md) is the contract the code is checked against.

## Guides

- [Writing interfaces](../AGENTS.md): views and updates, layout, keys, widgets, ripples, commands,
  materials, and how to verify with `render`.
- [Development](guides/development.md): build, test, run the tools, and work on the framework and the
  website together.

## Recipes

Each recipe is a page in [recipes/](recipes/) and a file in `examples/`, compiled into the library as
`amenui::recipes` and rendered by the tests.

- [A form](recipes/form.md), [a list with selection](recipes/list.md), [a dialog](recipes/dialog.md)
- [A counter with recomposition and a scoped ripple](recipes/counter.md)
- [A page change with a shared part](recipes/flow.md), [morph A into B](recipes/morph.md),
  [a chapter with actors that flow in](recipes/chapter.md)
- [A material on one element](recipes/material.md), [a preset with its own transition](recipes/preset.md),
  [the annotation layer on a subtree](recipes/annotate.md)
- [A 3D object in a card](recipes/mesh.md), [a stack of cards](recipes/stack.md),
  [a dot chart](recipes/chart.md)
- [Gestures: a bottom sheet and a floating box](recipes/gesture.md), [a menu with submenus](recipes/submenu.md),
  [a list that loads](recipes/loading.md)

## Reference

- [Architecture](reference/architecture.md): the pipeline, each module's responsibility, the binaries.
- [Fonts](reference/fonts.md): the bundled faces, where they come from, and what they cover.
- [registry.json](../registry.json): every widget's files, dependencies, response and recipe.
- [llms.txt](../llms.txt): a short index for language models.

## Design

- [DESIGN.md](DESIGN.md): the pipeline, materials, input, transforms, performance, accessibility,
  languages, components, styles and scenes, with the numbers and acceptance checks for each.
  Code comments cite its sections (`DESIGN.md §1.8.2`).
- [Changelog](../CHANGELOG.md): what changed, newest first.
