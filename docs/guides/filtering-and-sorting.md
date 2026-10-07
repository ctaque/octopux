---
title: Filtering and sorting
parent: Guides
nav_order: 3
---

# Filtering and sorting

`SqlxFilter` turns the `Option` fields of the list query into `WHERE` conditions. The field name gives the column and the operator (suffixes `_ne`, `_gt`, `_gte`, `_lt`, `_lte`, `_like`):

```rust
#[derive(Deserialize, SqlxFilter)]
#[sqlx_filter(database = "postgres")]
pub struct ListQuery {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    pub name: Option<String>,      // ?name=foo        → name = 'foo'
    pub price_gte: Option<i32>,    // ?price_gte=10    → price >= 10
    #[sqlx_filter(column = "name", op = "like")]
    pub q: Option<String>,         // ?q=alpha%        → name LIKE 'alpha%'
    #[sqlx_filter(sort = "name, price")]
    pub sort: Option<String>,      // ?sort=-price,name (only these columns)
    #[sqlx_filter(sort_direction)]
    pub order: Option<String>,     // ?order=desc
}
```

Add `filter` to the `#[sqlx_model]` of the model to apply it in `list`. Values are always bound, never written in the SQL. A sort column not allowed, a repeated column or a direction other than `asc`/`desc` answers 400 `BAD_REQUEST`, see [Errors]({{ '/guides/errors/' | relative_url }}).
