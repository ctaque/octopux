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
| `timestamps` | Fill `created_at` and `updated_at` automatically |
| `soft_delete` | `delete` sets `deleted_at` instead of removing the row, which is then hidden |
| `default_limit`, `max_limit` | The page size of `list` |
| `filter` | `list` applies the filters and sort of its query, see [`SqlxFilter`]({{ '/guides/filtering-and-sorting/' | relative_url }}) |
| `before_save` | Transform the payload before writing it, see [Transforming the payload]({{ '/guides/before-save/' | relative_url }}) |
