# recipes example

A semantic search over 340 recipes of 17 cuisines: ask for "something warm and comforting for a
cold winter evening" and get a mulled wine, a bean soup and a boeuf bourguignon, although none of
them contains these words.

The embeddings are real ones, computed locally by the BGE small English v1.5 model (384 dimensions)
with [fastembed](https://crates.io/crates/fastembed), without an API key. The recipes are served
by the sqlx derives (octopux `pgvector` feature) on REST routes documented with OpenAPI, and on a
GraphQL schema.

```bash
docker compose up -d   # PostgreSQL with pgvector on localhost:5434
cargo run              # migrates, seeds 340 recipes, embeds them, listens on 127.0.0.1:8085
```

The first run downloads the model, about 130 MB, into `.fastembed_cache`, then computes the
embeddings of the seeded recipes in a few seconds (`embedded 340 recipes`). The next runs load the
model from the cache and embed nothing.

| Route | Action |
| --- | --- |
| `GET /v1/recipe/search?q=...` | Search the recipes by meaning, with the filters of the list |
| `GET /v1/recipe` | List a page of recipes, filtered by `cuisine`, `course`, `vegetarian`, `minutes_lte`, ordered by `near` |
| `GET /v1/recipe/{id}` | Find a recipe |
| `POST /v1/recipe` | Create a recipe, embedded before the insert |
| `PUT /v1/recipe/{id}` | Update a recipe, embedded again before the update |
| `DELETE /v1/recipe/{id}` | Delete a recipe (soft delete) |
| `GET /v1/recipe/{id}/similar` | The other recipes, the most similar first, with their similarity |

Swagger UI is served on `/swagger`. The GraphQL schema is served on `POST /graphql`, GraphiQL on
`GET /graphql`: see [GraphQL](#graphql).

## How it works

```
                 text of the recipe                         vector(384)
POST /recipe ──▶ before_save ──▶ BGE small (fastembed) ──▶ recipe.embedding
                                                                 │ HNSW index
GET /recipe/search?q=... ──▶ BGE small ──▶ ListQuery { near } ───┘ ORDER BY embedding <=> $1
```

1. **The table** (`migrations/`): a `recipe` table with an `embedding vector(384)` column, NULL until
   computed, and an HNSW index for the cosine distance. The seed migration inserts the 340 recipes
   without their embedding.
2. **At the start** (`src/embedder.rs`): `embed_missing` embeds the recipes whose embedding is NULL,
   64 at a time, and writes each batch with a single `UPDATE ... FROM UNNEST(...)`. The embedded
   text is the name, the cuisine, the course and the description:
   `Miso soup, a vegetarian japanese soup. Light dashi broth with miso paste...`
3. **On create and update** (`src/recipe.rs`): the payload is the text of the recipe only. The
   `embedding` field of `NewRecipe` and `UpdatableRecipe` is `#[serde(skip)]`, filled by their
   `BeforeSave` hook, so a new or edited recipe is found by the searches at once. The `Recipe`
   model has no `embedding` field: the 384 numbers stay in the database.
4. **The search** (`src/recipe_search.rs`): `GET /recipe/search` embeds `q`, then calls
   `Recipe::list` with `near` set to its vector. `SqlxFilter` applies the filters in `WHERE` and
   orders the rows by `embedding <=> $1`, nearest first.
5. **The similar recipes** (`src/recipe_similar.rs`): a `HasMany` relation reading the embedding of
   the recipe and ordering the others by their distance to it, with `1 - distance` as similarity.

The model runs on the CPU: `TextEmbedding::embed` takes `&mut self`, so the workers share it behind
a mutex, and it runs on the blocking threads of the runtime. A search embeds one short text, a few
milliseconds.

## Searching

`q` is what you feel like eating, in English. The other parameters filter the recipes:

| Parameter | Filter | Example |
| --- | --- | --- |
| `q` | **Required.** Ordered by the cosine distance to its embedding | `q=dessert with chocolate` |
| `cuisine` | `cuisine = ...` | `cuisine=korean` |
| `course` | `starter`, `soup`, `main`, `side`, `dessert`, `breakfast` or `drink` | `course=soup` |
| `vegetarian` | `vegetarian = ...` | `vegetarian=true` |
| `minutes_lte` | `minutes <= ...` | `minutes_lte=20` |
| `limit`, `offset` | The page, 20 recipes by default, 100 at most | `limit=5` |

```bash
# by meaning, not by words
curl -G localhost:8085/v1/recipe/search --data-urlencode 'q=something warm and comforting for a cold winter evening' -d limit=5
# Glogg, Mulled wine, Bissara, Shepherds pie, Boeuf bourguignon

curl -G localhost:8085/v1/recipe/search --data-urlencode 'q=dessert with chocolate' -d limit=4
# Brownies, Bread and butter pudding, Chocolate mousse, Churros con chocolate

# the filters apply before the order: vegetarian, ready in 20 minutes at most
curl -G localhost:8085/v1/recipe/search --data-urlencode 'q=quick refreshing summer dish' -d vegetarian=true -d minutes_lte=20 -d limit=5
# Morning glory stir fry, Morning glory with garlic, Mint lemonade, Caprese salad, Stir fried bok choy

curl -G localhost:8085/v1/recipe/search --data-urlencode 'q=spicy noodle soup' -d cuisine=korean -d limit=3
# Sundubu jjigae, Kimchi jjigae, Naengmyeon
```

An empty `q` is answered 400 `{"code": "BAD_REQUEST", "message": "`q` must tell what to search"}`.

`GET /v1/recipe` takes the same filters, a `sort` (`name`, `cuisine`, `minutes`, `created_at`, `-`
for descending) and `near`, a vector of 384 numbers computed elsewhere:

```bash
curl 'localhost:8085/v1/recipe?cuisine=greek&course=dessert&sort=minutes'
# Loukoumades, Baklava, Galaktoboureko
```

## Similar recipes

```bash
# the recipes closest to the tonkotsu ramen (id 41)
curl 'localhost:8085/v1/recipe/41/similar?limit=4'
# [{"id":67,"name":"Wonton soup","cuisine":"chinese","course":"soup","similarity":0.82}, Samgyetang 0.786,
#  Udon noodle soup 0.785, Hot and sour soup 0.779]

# the same dish elsewhere in the world: only the recipes of another cuisine
curl 'localhost:8085/v1/recipe/1/similar?other_cuisine=true&limit=4'

# only the recipes at least 80 % similar
curl 'localhost:8085/v1/recipe/41/similar?min_similarity=0.8'
```

With this model, unrelated recipes are still around 0.5 similar: compare the similarities with each
other rather than with a fixed scale.

## Adding a recipe

The payload has no embedding. `before_save` computes it from the text:

```bash
curl -X POST localhost:8085/v1/recipe -H 'content-type: application/json' -d '{
  "name": "Chilled soba",
  "cuisine": "japanese",
  "course": "main",
  "vegetarian": true,
  "minutes": 15,
  "description": "Cold buckwheat noodles with a soy dipping sauce, scallions and wasabi, for hot summer days."
}'

curl -G localhost:8085/v1/recipe/search --data-urlencode 'q=cold noodles when it is very hot outside' -d limit=3
# Naengmyeon, Chilled soba, Kimchi jjigae
```

A `PUT` embeds the new text again, so the recipe moves in the searches with its description.

## GraphQL

The schema (`src/graphql.rs`) serves the same recipes as the REST routes, the similar recipes being
a field of `Recipe`. The embedding is never a field: the inputs of the mutations are the text of the
recipe, embedded by `before_save` as for `POST` and `PUT`.

| Field | REST route |
| --- | --- |
| `searchRecipes(q, offset, limit, cuisine, course, vegetarian, minutesLte)` | `GET /v1/recipe/search` |
| `recipes(offset, limit, cuisine, course, vegetarian, minutesLte, near, sort)` | `GET /v1/recipe`, `near` being the `Vector` scalar |
| `recipe(id)` | `GET /v1/recipe/{id}` |
| `Recipe.similar(minSimilarity, otherCuisine, limit)` | `GET /v1/recipe/{id}/similar` |
| `createRecipe(input: NewRecipe!)` | `POST /v1/recipe` |
| `updateRecipe(input: UpdatableRecipe!)` | `PUT /v1/recipe/{id}` |
| `deleteRecipe(id)` | `DELETE /v1/recipe/{id}` |

```graphql
# by meaning, the filters applying first
{ searchRecipes(q: "spicy noodle soup", cuisine: "korean", limit: 3) { id name } }
# Sundubu jjigae, Kimchi jjigae, Naengmyeon

# a recipe and the recipes closest to it, in one query
{
  recipe(id: 41) {
    name
    similar(limit: 3) { name cuisine similarity }
  }
}
# Tonkotsu ramen: Wonton soup 0.82, Samgyetang 0.786, Udon noodle soup 0.785

# the input has no embedding, `before_save` computes it
mutation {
  createRecipe(input: {
    name: "Chilled soba"
    cuisine: "japanese"
    course: "main"
    vegetarian: true
    minutes: 15
    description: "Cold buckwheat noodles with a soy dipping sauce, scallions and wasabi, for hot summer days."
  }) { id name createdAt }
}

# soft delete, the recipe is returned with its deletedAt
mutation { deleteRecipe(id: 341) { id deletedAt } }
```

```bash
curl localhost:8085/graphql -H 'content-type: application/json' \
  -d '{"query": "{ searchRecipes(q: \"dessert with chocolate\", limit: 2) { name } }"}'
# {"data":{"searchRecipes":[{"name":"Brownies"},{"name":"Bread and butter pudding"}]}}
```

An empty `q` is answered with the error "`q` must tell what to search", an unknown recipe with
`ENTITY_NOT_FOUND`. `similar` runs one query per recipe: ask for it on a recipe or a short list.

## Going further

- **Another model**: change `EmbeddingModel::BGESmallENV15` in `src/embedder.rs` and the size of
  the column, `vector(384)`, in the migration. A multilingual model, `MultilingualE5Small`, would
  answer searches in French over English recipes. Recompute the embeddings by setting them to NULL:
  `UPDATE recipe SET embedding = NULL`, then restart.
- **A hosted model**: replace `Embedder` by a call to an embedding API (OpenAI
  `text-embedding-3-small`, 1536 dimensions, `vector(1536)`). The routes do not change.
- **Hybrid search**: combine the distance with a keyword match, a `tsvector` column and `@@`, in a
  custom query as `recipe_similar.rs` does.
- **Reset the data**: `docker compose down -v`, then `cargo run`.
