# recipes example

A semantic search over 340 recipes of 17 cuisines: ask for "something warm and comforting for a
cold winter evening", or in French for "quelque chose de chaud et réconfortant pour une soirée
d'hiver", and get a mulled wine, a glogg and a boeuf bourguignon, although none of them contains
these words, and the recipes are written in English.

The embeddings are real ones, computed locally by the multilingual E5 small model (384 dimensions)
with [fastembed](https://crates.io/crates/fastembed), without an API key. The recipes are served
by the sqlx derives (octopux `pgvector` feature) on REST routes documented with OpenAPI, and on a
GraphQL schema.

```bash
docker compose up -d   # PostgreSQL with pgvector on localhost:5434
cargo run              # migrates, seeds 340 recipes, embeds them, listens on 127.0.0.1:8085
```

The first run downloads the model, about 470 MB, into `.fastembed_cache`, then computes the
embeddings of the seeded recipes in a few seconds (`embedded 340 recipes`). The next runs load the
model from the cache and embed nothing.

| Route | Action |
| --- | --- |
| `GET /` | The search page: type what you feel like eating, the results follow as you type |
| `GET /v1/recipe/search?q=...` | Search the recipes by meaning, with the filters of the list |
| `GET /v1/recipe` | List a page of recipes, filtered by `cuisine`, `course`, `vegetarian`, `minutes_lte` |
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
POST /recipe ──▶ before_save ──▶ E5 small (fastembed) ───▶ recipe.embedding
                                                                 │ HNSW index
GET /recipe/search?q=... ──▶ E5 small ───▶ ListQuery { near } ───┘ ORDER BY embedding <=> $1
```

1. **The table** (`migrations/`): a `recipe` table with an `embedding vector(384)` column, NULL until
   computed, and an HNSW index for the cosine distance. The seed migration inserts the 340 recipes
   without their embedding.
2. **At the start** (`src/embedder.rs`): `embed_missing` embeds the recipes whose embedding is NULL,
   64 at a time, and writes each batch with a single `UPDATE ... FROM UNNEST(...)`. The embedded
   text is the name, the cuisine, the course and the description, after the `passage: ` prefix
   E5 was trained with: `passage: Miso soup, a vegetarian japanese soup. Light dashi broth...`
3. **On create and update** (`src/recipe.rs`): the payload is the text of the recipe only. The
   `embedding` field of `NewRecipe` and `UpdatableRecipe` is `#[serde(skip)]`, filled by their
   `BeforeSave` hook, so a new or edited recipe is found by the searches at once. The `Recipe`
   model has no `embedding` field: the 384 numbers stay in the database.
4. **The search** (`src/recipe_search.rs`): `GET /recipe/search` embeds `q` after the `query: `
   prefix, then calls
   `Recipe::list` with `near` set to its vector. `SqlxFilter` applies the filters in `WHERE` and
   orders the rows by `embedding <=> $1`, nearest first.
5. **The similar recipes** (`src/recipe_similar.rs`): a `HasMany` relation reading the embedding of
   the recipe and ordering the others by their distance to it, with `1 - distance` as similarity.

The model runs on the CPU: `TextEmbedding::embed` takes `&mut self`, so the workers share it behind
a mutex, and it runs on the blocking threads of the runtime. A search embeds one short text, a few
milliseconds.

## The search page

Open <http://127.0.0.1:8085/>. The page (`static/index.html`, compiled into the binary) sends the
text and the filters to `GET /v1/recipe/search`. The backend embeds the text, then lists the
recipes nearest to it. The browser never sees a vector: it sends text and receives recipes. A
click on a recipe calls `GET /v1/recipe/{id}/similar`, which reuses the stored embedding of the
recipe and embeds nothing.

## Searching

`q` is what you feel like eating, in English, in French or in another of the languages of the
model: a text and its translation have close embeddings, so a French search finds the recipes
written in English. The other parameters filter the recipes:

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
# Janssons temptation, Glogg, Boeuf bourguignon, Ayib, Mulled wine

curl -G localhost:8085/v1/recipe/search --data-urlencode 'q=dessert with chocolate' -d limit=4
# Chocolate mousse, Brownies, Chocolate chip cookies, Churros

# the filters apply before the order: vegetarian, ready in 20 minutes at most
curl -G localhost:8085/v1/recipe/search --data-urlencode 'q=quick refreshing summer dish' -d vegetarian=true -d minutes_lte=20 -d limit=5
# Fattoush, Cucumber sunomono, Eton mess, Hummus, Mint lemonade

curl -G localhost:8085/v1/recipe/search --data-urlencode 'q=spicy noodle soup' -d cuisine=korean -d limit=3
# Sundubu jjigae, Kimchi jjigae, Doenjang jjigae

# in French, over the same recipes written in English
curl -G localhost:8085/v1/recipe/search --data-urlencode "q=quelque chose de chaud et réconfortant pour une soirée d'hiver" -d limit=5
# Janssons temptation, Glogg, Vichyssoise, Mulled wine, Sikhye

curl -G localhost:8085/v1/recipe/search --data-urlencode 'q=un dessert au chocolat' -d limit=4
# Brownies, Chocolate mousse, Churros, Chocolate chip cookies

curl -G localhost:8085/v1/recipe/search --data-urlencode 'q=soupe de nouilles épicée' -d cuisine=korean -d limit=3
# Sundubu jjigae, Kimchi jjigae, Doenjang jjigae
```

An empty `q` is answered 400 `{"code": "BAD_REQUEST", "message": "`q` must tell what to search"}`.

`GET /v1/recipe` takes the same filters and a `sort` (`name`, `cuisine`, `minutes`, `created_at`, `-`
for descending). It takes no vector: `near` is set by the search only, from the embedding of `q`.

```bash
curl 'localhost:8085/v1/recipe?cuisine=greek&course=dessert&sort=minutes'
# Loukoumades, Baklava, Galaktoboureko
```

## Similar recipes

`GET /v1/recipe/{id}/similar` lists the other recipes, the most similar first. It is a `HasMany`
relation (`src/recipe_similar.rs`), documented in Swagger UI under the `recipe` tag. It reads the
stored embedding of the recipe and embeds nothing: no model call, a single indexed query.

| Parameter | Filter | Example |
| --- | --- | --- |
| `min_similarity` | The least cosine similarity, from -1 to 1, none by default | `min_similarity=0.9` |
| `other_cuisine` | Only the recipes of another cuisine with `true` | `other_cuisine=true` |
| `limit` | 5 recipes by default, 100 at most | `limit=4` |

Each recipe is returned with its `id`, `name`, `cuisine`, `course` and `similarity`, `1 - (embedding <=> ...)`.
An unknown or deleted recipe is answered 404 `{"code": "ENTITY_NOT_FOUND", ...}`. A recipe without an
embedding yet is answered 404 as well.

```bash
# the recipes closest to the tonkotsu ramen (id 41)
curl 'localhost:8085/v1/recipe/41/similar?limit=4'
# [{"id":45,"name":"Tonkatsu","cuisine":"japanese","course":"main","similarity":0.916}, Sundubu jjigae 0.912,
#  Udon noodle soup 0.910, Wonton soup 0.902]

# the same dish elsewhere in the world: only the recipes of another cuisine
curl 'localhost:8085/v1/recipe/1/similar?other_cuisine=true&limit=4'
# Spaghetti carbonara: Pastitsio 0.880, Moussaka 0.860, Spanakopita 0.851, Pisto manchego 0.848

# only the recipes at least 90.5 % similar
curl 'localhost:8085/v1/recipe/41/similar?min_similarity=0.905'
# Tonkatsu 0.916, Sundubu jjigae 0.912, Udon noodle soup 0.910
```

With this model, unrelated recipes are still around 0.8 similar: compare the similarities with each
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
# Naengmyeon, Chilled soba, Hotteok
curl -G localhost:8085/v1/recipe/search --data-urlencode 'q=nouilles froides quand il fait très chaud' -d limit=3
# Naengmyeon, Chilled soba, Chow mein
```

A `PUT` embeds the new text again, so the recipe moves in the searches with its description.

## GraphQL

The schema (`src/graphql.rs`) serves the same recipes as the REST routes, the similar recipes being
a field of `Recipe`. The embedding is never a field: the inputs of the mutations are the text of the
recipe, embedded by `before_save` as for `POST` and `PUT`.

| Field | REST route |
| --- | --- |
| `searchRecipes(q, offset, limit, cuisine, course, vegetarian, minutesLte)` | `GET /v1/recipe/search` |
| `recipes(offset, limit, cuisine, course, vegetarian, minutesLte, sort)` | `GET /v1/recipe` |
| `recipe(id)` | `GET /v1/recipe/{id}` |
| `Recipe.similar(minSimilarity, otherCuisine, limit)` | `GET /v1/recipe/{id}/similar` |
| `createRecipe(input: NewRecipe!)` | `POST /v1/recipe` |
| `updateRecipe(input: UpdatableRecipe!)` | `PUT /v1/recipe/{id}` |
| `deleteRecipe(id)` | `DELETE /v1/recipe/{id}` |

`similar` is not a root query: in the documentation explorer of GraphiQL, it is a field of the
`Recipe` type, after `deletedAt`, so it is asked for on any recipe, whether it comes from `recipe`,
`recipes` or `searchRecipes`. Its arguments are those of the REST route, in camelCase, and a recipe
without an embedding yet has no similar recipe, `[]`. Each recipe of a list runs its own query (no
DataLoader), so keep the `limit` of the list small when asking for `similar`.

```graphql
# by meaning, the filters applying first
{ searchRecipes(q: "spicy noodle soup", cuisine: "korean", limit: 3) { id name } }
# Sundubu jjigae, Kimchi jjigae, Doenjang jjigae

# a recipe and the recipes closest to it, in one query
{
  recipe(id: 41) {
    name
    similar(limit: 3) { name cuisine similarity }
  }
}
# Tonkotsu ramen: Tonkatsu 0.916, Sundubu jjigae 0.912, Udon noodle soup 0.910

# the similar recipes of each result of a search, here from another cuisine
{
  searchRecipes(q: "dessert with chocolate", limit: 2) {
    name
    similar(limit: 2, otherCuisine: true) { name cuisine similarity }
  }
}
# Chocolate mousse: Mochi 0.894, Chocolate chip cookies 0.888
# Brownies: Churros 0.897, Mochi 0.887

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
# {"data":{"searchRecipes":[{"name":"Chocolate mousse"},{"name":"Brownies"}]}}
```

An empty `q` is answered with the error "`q` must tell what to search", an unknown recipe with
`ENTITY_NOT_FOUND`. `similar` runs one query per recipe: ask for it on a recipe or a short list.

## Going further

- **Another model**: change `EmbeddingModel::MultilingualE5Small` in `src/embedder.rs`, its
  prefixes (`query: ` and `passage: ` for E5, an instruction before the query only for BGE), and
  the size of the column, `vector(384)`, in a new migration. Recompute the embeddings by setting
  them to NULL, `UPDATE recipe SET embedding = NULL`, as `20261009000002_reembed_multilingual.sql`
  does, then restart. `BGESmallENV15` is smaller (130 MB) but answers in English only.
- **A hosted model**: replace `Embedder` by a call to an embedding API (OpenAI
  `text-embedding-3-small`, 1536 dimensions, `vector(1536)`). The routes do not change.
- **Hybrid search**: combine the distance with a keyword match, a `tsvector` column and `@@`, in a
  custom query as `recipe_similar.rs` does.
- **Reset the data**: `docker compose down -v`, then `cargo run`.
