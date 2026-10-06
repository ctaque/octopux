---
title: Reverse engineering
nav_order: 7
---

# Reverse engineering a database

`octopux-reverse` reads an existing database and generates the matching models and relations, by calling the `octopux` CLI for you.

```bash
cargo install octopux-reverse

# print the commands to review them, then run them
octopux-reverse --database-url postgres://localhost/my_db --openapi > reverse.sh
sh reverse.sh

# or run them directly
octopux-reverse --database-url postgres://localhost/my_db --openapi --run
```

## What you get

- **One model per table** with an `id` primary key (`books` gives `Book`), one field per column. Nullable columns become `Option<T>`.
- **One relation per foreign key** to another table's `id` (`GET /author/{id}/books`), and **many-to-many relations** for join tables.
- `--timestamps` when the table has `created_at`, `updated_at` and `deleted_at` columns.
- At the end, the `mod` and `.configure` lines to paste into `src/main.rs`.

Columns that cannot be mapped to a Rust type are skipped with a warning.

## Options

| Option | Description |
| --- | --- |
| `--database-url` | The database to read, `DATABASE_URL` by default |
| `--migrations <dir>` | Build the schema from sqlx migrations instead of a live database |
| `--schema <name>` | Read the tables of this PostgreSQL schema instead of the current one (`public`). Needs a `DATABASE_URL` or `--database-url` with a `postgresql:` connection string scheme |
| `--tables`, `--exclude` | Only read, or skip, these tables (comma separated) |
| `--no-relations` | Only generate the models |
| `--run` | Run the commands instead of printing them |
| `--openapi`, `--graphql`, `--output`, `--force` | Passed to `octopux` |
