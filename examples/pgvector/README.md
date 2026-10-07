# pgvector example

Articles with a `vector(4)` embedding and optional `sparsevec(8)` keywords, served as JSON by the
sqlx derives (octopux `pgvector` feature), on REST routes documented with OpenAPI and in a GraphQL
schema, where the vectors are scalars (octopux `graphql` feature). The list is a similarity search.

The embeddings place each article on 4 readable axes, `[cooking, sport, tech, travel]`, where an
embedding model would use hundreds of them, so that the example runs without one. The keywords
weigh the words of a vocabulary of 8: `pasta, football, rust, database, japan, mountain, recipe, ai`.

```bash
docker compose up -d   # PostgreSQL with pgvector on localhost:5433
cargo run              # migrates, seeds 8 articles, listens on 127.0.0.1:8085
```

| Route | Action |
| --- | --- |
| `GET /v1/article` | List a page of articles, filtered by `category`, ordered by `near`, `near_l2` or `keywords` |
| `GET /v1/article/{id}` | Find an article |
| `POST /v1/article` | Create an article |
| `PUT /v1/article/{id}` | Update an article |
| `DELETE /v1/article/{id}` | Delete an article |
| `GET /v1/article/{id}/similar?limit=3` | The other articles, the most similar first, with their similarity |

Swagger UI is served on `/swagger`, GraphiQL on `GET /graphql`.

The list is ordered by the distance of the vectors to the one of its query string (`SqlxFilter`
derive, `op = "nearest"`), nearest first, then by `sort`; `limit` keeps the `k` nearest articles:

| Parameter | Order | Example |
| --- | --- | --- |
| `near=[cooking,sport,tech,travel]` | `embedding <=> $1`, cosine distance | `?near=0.8,0,0,0.6` |
| `near_l2=[...]` | `embedding <-> $1`, Euclidean distance | `?near_l2=[0,0,1,0]` |
| `keywords={index:weight,...}/8` | `keywords <#> $1`, the largest inner product first | `?keywords={5:1,6:1}/8` |

```bash
# the articles about cooking and travel
curl 'localhost:8085/v1/article?near=0.8,0,0,0.6&limit=3'
# Ramen in Tokyo, Carbonara in ten minutes, Hiking in the Japanese Alps

# the tech articles, the closest to pure tech first
curl 'localhost:8085/v1/article?category=tech&near=[0,0,1,0.1]'

# the articles about japan and mountains, the sparse vector having its indices from 1
curl -G localhost:8085/v1/article --data-urlencode 'keywords={5:1,6:1}/8' --data-urlencode limit=3
# Hiking in the Japanese Alps, Trail running in the Alps, Ramen in Tokyo

# the articles similar to Ramen in Tokyo
curl 'localhost:8085/v1/article/2/similar?limit=3'
# [{"id":1,"title":"Carbonara in ten minutes","category":"cooking","similarity":0.776...}, ...]

# the articles at least 90 % similar to Postgres as a vector database
curl 'localhost:8085/v1/article/4/similar?min_similarity=0.9'

curl -X POST localhost:8085/v1/article -H 'content-type: application/json' -d '{
  "title": "Sushi at home",
  "category": "cooking",
  "embedding": [0.9, 0, 0, 0.3],
  "keywords": { "dimensions": 8, "indices": [4, 6], "values": [1, 1] }
}'
```

In JSON, a sparse vector is `{"dimensions", "indices", "values"}` with its indices from 0
(`japan` is 4); in a query string or as a GraphQL string, it is the literal of pgvector,
`{5:1,7:1}/8`, with its indices from 1.

A vector of another size than its column (`?near=1,0`, an embedding of 2 numbers) is answered 400
`BAD_REQUEST`, as is an invalid vector (`?near=1,x`).

## GraphQL

The schema is served on `POST /graphql`. The vectors are the `Vector` and `SparseVector` scalars:
a list of numbers and an object, as in the REST routes, or the literal of pgvector as a string.

### Queries

```graphql
# an article and its vectors, the sparse vector as JSON with its indices from 0
{ article(id: 2) { title embedding keywords } }

# the 3 articles about cooking and travel, by cosine distance
{ articles(near: [0.8, 0, 0, 0.6], limit: 3) { title category } }

# the tech articles, the closest to pure tech first
{ articles(category: "tech", near: [0, 0, 1, 0.1]) { id title } }

# the articles about japan and mountains, the sparse vector as the literal of pgvector
{ articles(keywords: "{5:1,6:1}/8", limit: 3) { title keywords } }

# the vector as a variable, of the Vector type
query Near($near: Vector!, $limit: Int) {
  articles(near: $near, limit: $limit) { id title embedding }
}

# the sparse vector as a variable, of the SparseVector type
query Keywords($keywords: SparseVector!) {
  articles(keywords: $keywords, limit: 2) { title }
}
```

The variables of `Near`, the articles nearest to sport, and of `Keywords`, pasta and recipe as JSON
(indices from 0):

```json
{ "near": [0, 1, 0, 0.2], "limit": 2 }
```

```json
{ "keywords": { "dimensions": 8, "indices": [0, 6], "values": [1, 1] } }
```

```json
{"data":{"articles":[{"id":6,"title":"The World Cup final","embedding":[0.0,1.0,0.1,0.1]},{"id":3,"title":"Trail running in the Alps","embedding":[0.0,0.9,0.05,0.5]}]}}
```

```json
{"data":{"articles":[{"title":"Carbonara in ten minutes"},{"title":"Ramen in Tokyo"}]}}
```

### Mutations

```graphql
# an article, its keywords as the literal of pgvector (indices from 1)
mutation {
  createArticle(input: {
    title: "Pasta al pesto"
    category: "cooking"
    embedding: [0.95, 0, 0, 0.05]
    keywords: "{1:1,7:1}/8"
  }) { id title embedding keywords createdAt }
}

# the input as a variable; the update replaces every column, keywords left out become null
mutation Move($input: UpdatableArticle!) {
  updateArticle(input: $input) { id title embedding keywords updatedAt }
}

# soft delete, the article is returned with its deletedAt
mutation { deleteArticle(id: 9) { id title deletedAt } }
```

```json
{"data":{"createArticle":{"id":9,"title":"Pasta al pesto","embedding":[0.95,0.0,0.0,0.05],"keywords":{"dimensions":8,"indices":[0,6],"values":[1.0,1.0]},"createdAt":"2026-10-07T15:15:16.347989+00:00"}}}
```

The variables of `Move`, the article moved from cooking to travel, which then comes first in
`articles(near: [0, 0, 0, 1])`:

```json
{
  "input": {
    "id": 9,
    "title": "Pasta in Rome",
    "category": "travel",
    "embedding": [0.6, 0, 0, 0.8]
  }
}
```

### With curl

```bash
curl localhost:8085/graphql -H 'content-type: application/json' \
  -d '{"query": "{ article(id: 2) { title embedding keywords } }"}'
# {"data":{"article":{"title":"Ramen in Tokyo","embedding":[0.7,0.0,0.05,0.7],"keywords":{"dimensions":8,"indices":[4,6],"values":[1.0,0.5]}}}}

curl localhost:8085/graphql -H 'content-type: application/json' -d '{
  "query": "query Near($near: Vector!) { articles(near: $near, limit: 1) { id title } }",
  "variables": { "near": [0, 0, 1, 0] }
}'
# {"data":{"articles":[{"id":4,"title":"Postgres as a vector database"}]}}

curl localhost:8085/graphql -H 'content-type: application/json' -d '{
  "query": "mutation New($input: NewArticle!) { createArticle(input: $input) { id embedding keywords } }",
  "variables": { "input": {
    "title": "Sushi at home",
    "category": "cooking",
    "embedding": [0.9, 0, 0, 0.3],
    "keywords": { "dimensions": 8, "indices": [4, 6], "values": [1, 1] }
  } }
}'
```

An invalid vector is a GraphQL error: ``Failed to parse "Vector": invalid vector `[1,x]`, use [1.5,-2,0.3]``.

## With real embeddings

Compute the embedding of each article with a model (`text-embedding-3-small` gives 1536 dimensions),
store it in a `vector(1536)` column, and compute the embedding of the search text the same way
before filling `near`. pgvector indexes a `vector` of at most 2000 dimensions with HNSW, a `halfvec`
(`pgvector::HalfVector`) of at most 4000.
