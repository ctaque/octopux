---
title: CLI
nav_order: 6
has_children: true
permalink: /cli/
---

# CLI

Files are written in `src` when it exists, in the working directory otherwise.

| Command | Does |
| --- | --- |
| <code>octopux --bootstrap [--sqlite&#124;--postgres&#124;--mysql] [--openapi] [--graphql]</code> | Write `src/main.rs` and `src/helpers.rs`, and offer to install the dependencies, asking for the options not given without a database flag |
| <code>octopux generate-model --name Project --sqlite&#124;--postgres&#124;--mysql [OPTIONS]</code> | Write `project.rs` (and its migration) — see [generate-model]({{ '/cli/generate-model/' | relative_url }}) |
| <code>octopux generate-relation --parent Project --child Book --sqlite&#124;--postgres&#124;--mysql [OPTIONS]</code> | Write `project_books.rs`, the `books` of a project — see [generate-relation]({{ '/cli/generate-relation/' | relative_url }}) |
| <code>octopux add-field --model Project --sqlite&#124;--postgres&#124;--mysql [OPTIONS]</code> | Add fields to `project.rs` (and the migration adding their columns), keeping the code written in it — see [add-field]({{ '/cli/add-field/' | relative_url }}) |
| <code>octopux schema [OPTIONS]</code> | Draw the tables created by the migrations, their keys and relations — see [schema]({{ '/cli/schema/' | relative_url }}) |
