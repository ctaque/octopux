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

The vectors are computed and read by the backend only: the clients send text and receive rows, never an embedding nor a sparse vector of keywords. The backend embeds the text it receives, before a save or before a search, and keeps the vectors in the database.

## Vector fields

With the `pgvector` feature, the `vector`, `halfvec` and `sparsevec` columns of [pgvector](https://github.com/pgvector/pgvector) are fields of the sqlx models, read and written in the binary format of pgvector. `Vector` and `HalfVector` are arrays of numbers (`Deref` to `Vec<f32>`); a `HalfVector` is rounded to half precision in the database, which halves the column and its index. A `SparseVector` is its dimensions and its non-zero values with their indices, from 0.

The model sent to the clients leaves the vector out: the column stays in the database, and the rows are read without it.

```rust
#[derive(Serialize, Deserialize, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
#[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
#[sqlx_model(database = "postgres", filter)]
#[octopux_info(path = "article")]
pub struct Article {
    pub id: Id,
    pub title: String,
    pub body: String,
    // no `embedding`: the vector(384) column is never sent
}
```

The models of the inserts and of the updates hold the vector, skipped from the payload and from the GraphQL input, and `before_save` computes it from the text:

```rust
use octopux::pgvector::Vector;

#[derive(Serialize, Deserialize, InputObject, HttpCreate, SqlxNewModel)]
#[http_create(SaveQuery, AppState)]
#[sqlx_model(database = "postgres", model = "Article", before_save)]
pub struct NewArticle {
    pub title: String,
    pub body: String,
    /// Computed by `before_save`, never read from the payload
    #[serde(skip)]
    #[graphql(skip)]
    pub embedding: Option<Vector>,
}

#[async_trait]
impl BeforeSave<AppState> for NewArticle {
    async fn before_save(mut self: Self, state: &AppState) -> Result<Self> {
        self.embedding = state.embedder.embed(vec![format!("{}\n{}", self.title, self.body)]).await?.pop();
        Ok(self)
    }
}
```

A payload with an `embedding` is accepted, and its vector ignored. The same goes for an `UpdatableArticle`, embedded again before the update.

## Similarity search

[`SqlxFilter`]({{ '/guides/filtering-and-sorting/' | relative_url }}) orders a list by the distance of a column to the vector of the query, nearest first, with `op = "nearest"`; `limit` keeps the `k` nearest rows. The vector field is skipped from the query string and from the OpenAPI document: only the backend sets it.

```rust
#[derive(Default, Deserialize, JsonSchema, ApiComponent, SqlxFilter)]
#[sqlx_filter(database = "postgres")]
pub struct ListQuery {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    pub category: Option<String>,
    // Set by the search only, from the embedding of its text
    #[sqlx_filter(column = "embedding", op = "nearest")]
    #[serde(skip)]
    #[schemars(skip)]
    pub near: Option<pgvector::Vector>,
    #[sqlx_filter(sort = "title")]
    pub sort: Option<String>,                     // orders the rows at the same distance
}
```

A search route takes the text, embeds it, and lists the rows with the vector:

```rust
#[derive(Deserialize, JsonSchema, ApiComponent)]
pub struct SearchQuery {
    pub q: String,                                // ?q=ramen in tokyo&category=travel
    pub limit: Option<usize>,
    pub category: Option<String>,
}

pub async fn search_articles(query: SearchQuery, state: &AppState) -> anyhow::Result<Vec<Article>> {
    if query.q.trim().is_empty() {
        return Err(octopux::Error::BadRequest("`q` must tell what to search".into()).into());
    }
    let near = Some(state.embedder.embed_query(&query.q).await?);
    let list = ListQuery { limit: query.limit, category: query.category, near, ..ListQuery::default() };
    Ok(Article::list(&list, state).await?)
}

// before the routes of the endpoint, so that `search` is not taken for an `{id}`
pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
    cfg.route("/article/search", apistos::web::get().to(search));
}
```

| `distance` | Order | HNSW operator class |
| --- | --- | --- |
| `cosine` (default) | `column <=> value` | `vector_cosine_ops` |
| `l2` | `column <-> value` | `vector_l2_ops` |
| `inner_product` | `column <#> value`, the largest inner product first | `vector_ip_ops` |
| `l1` | `column <+> value` | `vector_l1_ops` |

The conditions of the other fields still apply, and the distance orders the rows before the columns of the `sort` field. A sparse vector of keywords is set the same way, by the backend, with `distance = "inner_product"`. Index the column with HNSW and the operator class of the distance (`halfvec_cosine_ops`, `sparsevec_cosine_ops`... for the other types), up to 2000 dimensions for a `vector`, 4000 for a `halfvec`:

```sql
CREATE INDEX article_embedding_idx ON article USING hnsw (embedding vector_cosine_ops);
```

## Similar rows

The rows similar to a row reuse its stored vector, and embed nothing: a [`HasMany`]({{ '/guides/relations/' | relative_url }}) relation reads the vector of the row, then orders the others by their distance to it, in a single indexed query.

```rust
async fn list_related(id: Id, query: &ArticleSimilarQuery, state: &AppState) -> Result<Option<Vec<SimilarArticle>>> {
    let Some(embedding) = sqlx::query_scalar::<_, Vector>("SELECT embedding FROM article WHERE id = $1 AND embedding IS NOT NULL")
        .bind(id)
        .fetch_optional(&state.pool)
        .await? else {
        return Ok(None);                          // 404 ENTITY_NOT_FOUND
    };
    let articles = sqlx::query_as::<_, SimilarArticle>(
        "SELECT id, title, 1 - (embedding <=> $2) AS similarity FROM article
         WHERE id <> $1 ORDER BY embedding <=> $2 LIMIT $3",
    )
    .bind(id)
    .bind(&embedding)
    .bind(query.limit.unwrap_or(5) as i64)
    .fetch_all(&state.pool)
    .await?;
    Ok(Some(articles))
}
```

`GET /v1/article/{id}/similar` answers the rows with their similarity, never their vector.

## GraphQL and OpenAPI

With the `graphql` feature too, the vectors are the async-graphql scalars `Vector`, `HalfVector` and `SparseVector`, and with `openapi` they are documented as arrays and objects. Keep them out of the schemas all the same: the resolvers take text, and the inputs skip their vector with `#[graphql(skip)]`.

```rust
#[Object]
impl ArticleQuery {
    async fn search_articles(&self, ctx: &Context<'_>, q: String, category: Option<String>, limit: Option<usize>) -> async_graphql::Result<Vec<Article>> {
        Ok(search_articles(SearchQuery { q, category, limit }, app_state(ctx)?).await?)
    }

    async fn articles(&self, ctx: &Context<'_>, category: Option<String>, limit: Option<usize>) -> async_graphql::Result<Vec<Article>> {
        let query = ListQuery { limit, category, near: None, ..ListQuery::default() };
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

```graphql
{ searchArticles(q: "ramen in tokyo", limit: 2) { id title } }

# the input has no embedding, `before_save` computes it
mutation {
  createArticle(input: { title: "Pasta al pesto", body: "Basil, pine nuts, parmesan..." }) { id title }
}
```

A test of the schema checks that no vector leaks:

```rust
let sdl = Schema::build(ArticleQuery, ArticleMutation, EmptySubscription).finish().sdl();
assert!(!sdl.contains("Vector"), "a vector is exposed in\n{}", sdl);
```

## CLI and reverse engineering

The CLI proposes the vectors in the field types with `--postgres` (`embedding:pgvector::Vector`) and asks their dimensions: the migration creates the `vector` extension, `vector(1536)` columns and their HNSW indexes for the cosine distance. [`octopux-reverse`]({{ '/reverse-engineering/' | relative_url }}) maps the `vector`, `halfvec` and `sparsevec` columns to these types. The generated models expose these fields: remove them from the model, and skip them in the inserts, the updates and the filters as above. See the [`recipes`](https://github.com/ctaque/octopux/tree/main/examples/recipes) example for a semantic search over 340 recipes, embedded by the backend with fastembed, and its search page.
