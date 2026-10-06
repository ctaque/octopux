---
title: generate-model
parent: CLI
nav_order: 1
---

# generate-model

| Option | Description |
| --- | --- |
| `--fields` | Ask interactively for the fields |
| `--sqlx` | Implement the queries with sqlx |
| `--migration` | Create the SQL migration of the table |
| `--foreign-keys` | Ask which table and column each field references |
| `--unique` | Ask whether each field is unique |
| `--timestamps` | Add `created_at`, `updated_at`, `deleted_at`, with soft delete |
| `--sqlite`, `--postgres`, `--mysql` | **Required.** The target database |
| `--openapi`, `--graphql` | Document the routes / serve the model with GraphQL |
| `--table` | The table name, the snake_case model name by default |
| `--schema <name>` | With `--postgres`, the schema of the table: the queries and the migration use `app.project`, the migration creates the schema if missing |
| `--output` | The folder of the file (`--output=src/models`) |
| `--force` | Overwrite an existing file |

## Entering fields

When asked for the fields, type `name:type` (e.g. `title:String`, or `stars:2` to pick from the menu). A trailing `?` makes the field optional (`stars:i32?`), `-` removes the last one, and an empty name ends the input:

```
$ octopux generate-model --name Project --fields --sqlite
? Field 1 name › title:String
  ✔ title: String
? Field 2 name › stars:i32?
  ✔ stars: Option<i32> (nullable)
? Field 3 name ›
```
