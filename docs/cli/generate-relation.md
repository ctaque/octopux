---
title: generate-relation
parent: CLI
nav_order: 2
---

# generate-relation

| Option | Description |
| --- | --- |
| `--parent`, `--child` | The two models |
| `--foreign-key` | The column of the child referencing the parent, `project_id` by default |
| `--through` | Many-to-many: the join model |
| `--name` | The route segment, `books` by default |
| `--sqlx`, `--migration`, `--timestamps`, `--openapi`, `--graphql`, database flags, `--schema`, `--output`, `--force` | As for [`generate-model`]({{ '/cli/generate-model/' | relative_url }}) |

```bash
# one-to-many: GET /project/{id}/books
octopux generate-relation --parent Project --child Book --sqlx --openapi --sqlite

# many-to-many through ProjectCategory: GET /project/{id}/categories
octopux generate-relation --parent Project --child Category --through ProjectCategory --sqlx --openapi --sqlite
```

Declare the file with `mod project_books;` and mount it with `.configure(project_books::configure)`.
