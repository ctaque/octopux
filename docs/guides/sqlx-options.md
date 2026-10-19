---
title: sqlx options
parent: Guides
nav_order: 1
---

# sqlx options

The sqlx derives take a `#[sqlx_model(...)]` attribute:

| Key | Description |
| --- | --- |
| `database` | **Required.** `"sqlite"`, `"postgres"` or `"mysql"` |
| `model` | **Required on `SqlxNewModel`.** The model returned by `save` |
| `table` | The table, the lowercase model name by default |
| `pool` | The field of `AppState` holding the pool, `pool` by default |
| `primary_key` | The primary key column, `id` by default (`primary_key = "version"`): a field of the model and of the updatable struct |
| `new_id` | **On `SqlxNewModel`.** The function generating the primary key on insert, `new_id = "uuid::Uuid::new_v4"`. Without it, the database generates the key, or the creatable struct gives it as a field |
| `timestamps` | Fill `created_at` and `updated_at` automatically |
| `soft_delete` | `delete` sets `deleted_at` instead of removing the row, which is then hidden |
| `default_limit`, `max_limit` | The page size of `list` |
| `filter` | `list` applies the filters and sort of its query, see [`SqlxFilter`]({{ '/guides/filtering-and-sorting/' | relative_url }}) |
| `before_save` | Transform the payload before writing it, see [Transforming the payload]({{ '/guides/before-save/' | relative_url }}) |
