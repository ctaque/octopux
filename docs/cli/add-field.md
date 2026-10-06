---
title: add-field
parent: CLI
nav_order: 3
---

# add-field

Asks for fields, as `generate-model --fields`, and inserts them in the `Project`, `NewProject` and `UpdatableProject` structs of `project.rs`, before the timestamps. The rest of the file is left as is, unlike `generate-model --force`. The sqlx derives, GraphQL and OpenAPI take the new fields from the structs.

| Option | Description |
| --- | --- |
| `--model` | The model, `Project` |
| `--migration` | Create the `ALTER TABLE ... ADD COLUMN` migration |
| `--foreign-keys`, `--unique` | As for [`generate-model`]({{ '/cli/generate-model/' | relative_url }}) |
| `--default` | The SQL default of the new non-optional columns on the existing rows (`--default "'none'"`), asked for each of them otherwise |
| `--sqlite`, `--postgres`, `--mysql` | **Required.** The target database |
| `--schema <name>` | With `--postgres`, the schema of the table: the migration alters `app.project` |
| `--output` | The folder of the file |

{: .warning }
A non-optional column needs a default for the rows already in the table, enter `?` instead to make the field optional. SQLite only adds a foreign key column when it is nullable, and only with a constant default (no `CURRENT_TIMESTAMP`).

```
$ octopux add-field --model Project --migration --sqlite
? Field 1 name › stars:i32
  ✔ stars: i32
? Field 2 name ›
? Default of `stars` for the existing rows › (SQL value, `?` makes the field optional) [0]
```
