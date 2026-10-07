---
title: pgvector
parent: Guides
nav_order: 8
---

# pgvector

Enable the feature, with `postgres` in the features of sqlx:

```bash
cargo add octopux --features sqlx,pgvector         # add graphql for the GraphQL scalars
```

## Vector fields

With the `pgvector` feature, the `vector`, `halfvec` and `sparsevec` columns of [pgvector](https://github.com/pgvector/pgvector) are fields of the sqlx models, read and written in the binary format of pgvector and sent to and received from the client as JSON:

```rust
use octopux::pgvector::{self, Vector};

#[derive(Serialize, Deserialize, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
#[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
#[sqlx_model(database = "postgres", filter)]
#[octopux_info(path = "article")]
pub struct Article {
    pub id: Id,
    pub title: String,
    pub embedding: Vector,                          // vector(1536) NOT NULL
    pub small: Option<pgvector::HalfVector>,        // halfvec(3072)
    pub keywords: Option<pgvector::SparseVector>,   // sparsevec(30522)
}
```

```json
{ "id": 1, "title": "Ramen in Tokyo", "embedding": [0.7, 0.0, 0.05, 0.7], "small": null, "keywords": { "dimensions": 8, "indices": [4, 6], "values": [1.0, 0.5] } }
```

`Vector` and `HalfVector` are arrays of numbers (`Deref` to `Vec<f32>`); a `HalfVector` is rounded to half precision in the database, which halves the column and its index. A `SparseVector` is its dimensions and its non-zero values with their indices, from 0. In a query string, they are the literals of pgvector: `[0.7,0,0.05,0.7]` (or `0.7,0,0.05,0.7`), and `{5:1,7:0.5}/8` with the indices from 1. A vector with no dimension, a value that is not finite, or a vector of another size than its column (`vector(1536)`) answers 400 `BAD_REQUEST`.

## Similarity search

[`SqlxFilter`]({{ '/guides/filtering-and-sorting/' | relative_url }}) orders a list by the distance of a column to the vector of the query, nearest first, with `op = "nearest"`; `limit` keeps the `k` nearest rows:

```rust
#[derive(Deserialize, SqlxFilter)]
#[sqlx_filter(database = "postgres")]
pub struct ListQuery {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    pub category: Option<String>,
    #[sqlx_filter(column = "embedding", op = "nearest")]
    pub near: Option<pgvector::Vector>,           // ?near=[0.8,0,0,0.6]&limit=5
    #[sqlx_filter(column = "keywords", op = "nearest", distance = "inner_product")]
    pub keywords: Option<pgvector::SparseVector>, // ?keywords={5:1,6:1}/8
    #[sqlx_filter(sort = "title")]
    pub sort: Option<String>,                     // orders the rows at the same distance
}
```

| `distance` | Order | HNSW operator class |
| --- | --- | --- |
| `cosine` (default) | `column <=> value` | `vector_cosine_ops` |
| `l2` | `column <-> value` | `vector_l2_ops` |
| `inner_product` | `column <#> value`, the largest inner product first | `vector_ip_ops` |
| `l1` | `column <+> value` | `vector_l1_ops` |

The conditions of the other fields still apply, and the distance orders the rows before the columns of the `sort` field. Index the column with HNSW and the operator class of the distance (`halfvec_cosine_ops`, `sparsevec_cosine_ops`... for the other types), up to 2000 dimensions for a `vector`, 4000 for a `halfvec`:

```sql
CREATE INDEX article_embedding_idx ON article USING hnsw (embedding vector_cosine_ops);
```

## GraphQL and OpenAPI

With the `graphql` feature too, the vectors are the async-graphql scalars `Vector`, `HalfVector` and `SparseVector`, the same JSON or the literal of pgvector. With `openapi`, they are documented as arrays and objects.

The models derive `SimpleObject` and `InputObject` with their vector fields, and a resolver takes a vector as argument, here to order the list as `ListQuery`:

```rust
#[Object]
impl ArticleQuery {
    async fn articles(&self, ctx: &Context<'_>, category: Option<String>, near: Option<Vector>, keywords: Option<SparseVector>, limit: Option<usize>) -> async_graphql::Result<Vec<Article>> {
        let query = ListQuery { limit, category, near, keywords, ..ListQuery::default() };
        Ok(Article::list(&query, app_state(ctx)?).await?)
    }
}

#[Object]
impl ArticleMutation {
    async fn create_article(&self, ctx: &Context<'_>, input: NewArticle) -> async_graphql::Result<Article> {
        Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
    }
}
```

The vectors are written as lists of numbers in the query or in the variables, the sparse vectors as objects (indices from 0) or as the literal of pgvector (indices from 1):

```graphql
query Near($near: Vector!) {
  articles(near: $near, limit: 2) { id title embedding }
}

{ articles(keywords: "{5:1,6:1}/8", limit: 3) { title keywords } }

mutation {
  createArticle(input: {
    title: "Pasta al pesto"
    category: "cooking"
    embedding: [0.95, 0, 0, 0.05]
    keywords: { dimensions: 8, indices: [0, 6], values: [1, 1] }
  }) { id embedding keywords }
}
```

```json
{ "near": [0, 1, 0, 0.2] }
```

```json
{"data":{"articles":[{"id":6,"title":"The World Cup final","embedding":[0.0,1.0,0.1,0.1]},{"id":3,"title":"Trail running in the Alps","embedding":[0.0,0.9,0.05,0.5]}]}}
```

An invalid vector is a GraphQL error: ``Failed to parse "Vector": invalid vector `[1,x]`, use [1.5,-2,0.3]``.

## CLI and reverse engineering

The CLI proposes the vectors in the field types with `--postgres` (`embedding:pgvector::Vector`) and asks their dimensions: the migration creates the `vector` extension, `vector(1536)` columns and their HNSW indexes for the cosine distance. [`octopux-reverse`]({{ '/reverse-engineering/' | relative_url }}) maps the `vector`, `halfvec` and `sparsevec` columns to these types. See the [`pgvector`](https://github.com/ctaque/octopux/tree/main/examples/pgvector) example.
