---
title: Home
layout: home
nav_order: 1
description: "Lightweight GraphQL/JSON CRUD operations for Actix Web.
permalink: /
---

# Octopux
{: .fs-9 }

Lightweight GraphQL/JSON CRUD operations for [Actix Web](https://actix.rs) (v4).
{: .fs-6 .fw-300 }

[Get started]({{ '/quick-start/' | relative_url }}){: .btn .btn-primary .fs-5 .mb-4 .mb-md-0 .mr-2 }
[View on GitHub](https://github.com/ctaque/octopux){: .btn .fs-5 .mb-4 .mb-md-0 }

---

## Features

- **5 REST routes per model**: find, list, create, update, delete
- **[sqlx](https://docs.rs/sqlx) persistence** (SQLite, PostgreSQL, MySQL), queries written for you
- **OpenAPI** documentation with [apistos](https://docs.rs/apistos) and Swagger UI
- **GraphQL** with [async-graphql](https://docs.rs/async-graphql) and GraphiQL
- **Has-many and many-to-many relations**
- **PostGIS** geometries as GeoJSON, with spatial filters
- **A CLI** generating models, relations and SQL migrations, even from an existing database

## In 30 seconds

```bash
cargo install octopux-cli
cargo init my-api && cd my-api
octopux --bootstrap
octopux generate-model --name Project --fields --sqlx --migration --timestamps --openapi --sqlite
cargo run
```

Continue with the [installation]({{ '/installation/' | relative_url }}) and the [quick start]({{ '/quick-start/' | relative_url }}).
