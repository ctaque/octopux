---
title: Installation
nav_order: 2
---

# Installation

```bash
cargo install octopux-cli   # the `octopux` CLI
cargo install sqlx-cli      # optional, for `sqlx migrate run`
```

The library, in your crate:

```bash
cargo add octopux --features sqlx,openapi
cargo add actix-web serde --features serde/derive
```

## Features

| Feature | Enables | Also add |
| --- | --- | --- |
| `sqlx` | The sqlx derives (`SqlxModel`, `SqlxFilter`...) | `sqlx --features runtime-tokio,sqlite,macros,migrate,chrono` |
| `openapi` | The documented routes | `apistos --features chrono,swagger-ui` and `schemars --rename schemars --package apistos-schemars` |
| `postgis` | The PostGIS geometries (`octopux::postgis`), see [PostGIS]({{ '/guides/postgis/' | relative_url }}) | `postgres` in the sqlx features |
| `pgvector` | The pgvector vectors (`octopux::pgvector`), see [pgvector]({{ '/guides/pgvector/' | relative_url }}) | `postgres` in the sqlx features |
| `graphql` | With `postgis` or `pgvector`, the geometries and the vectors as GraphQL scalars | `async-graphql` |

Add `chrono --features serde` when a model has date fields or uses `--timestamps`.

{: .tip }
`octopux --bootstrap` offers to install all these dependencies for you.
