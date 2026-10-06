---
title: Quick start
nav_order: 3
---

# Quick start

```bash
cargo init my-api && cd my-api
octopux --bootstrap
octopux generate-model --name Project --fields --sqlx --migration --timestamps --openapi --sqlite
```

1. `--bootstrap` asks for the database, OpenAPI and GraphQL (`octopux --bootstrap --sqlite --openapi` answers them with flags), then writes `src/main.rs` (an actix server on `127.0.0.1:8085`, a SQLite pool on `data.db`) and `src/helpers.rs` (the `AppState` holding the pool).
2. `generate-model` asks for the fields, then writes `src/project.rs` and the migration creating the `project` table.
3. In `src/main.rs`, declare the model with `mod project;`, mount it with `.configure(project::configure)` in the `v1` scope, and uncomment `sqlx::migrate!().run(&pool)`.

```bash
cargo run
```

## Generated routes

| Route | Action |
| --- | --- |
| `GET /v1/project` | List a page of projects |
| `GET /v1/project/{id}` | Find a project |
| `POST /v1/project` | Create a project |
| `PUT /v1/project/{id}` | Update a project |
| `DELETE /v1/project/{id}` | Delete a project |

Swagger UI is served on `/swagger`, the OpenAPI document on `/openapi.json`.

![Swagger UI]({{ '/assets/images/openapi.png' | relative_url }}){: width="586" }
