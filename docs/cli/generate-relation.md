---
title: generate-relation
parent: CLI
nav_order: 2
---

# generate-relation

| Option | Description |
| --- | --- |
| `--parent`, `--child` | The two models |
| `--foreign-key` | The column of the child referencing the parent, `project_id` by default |
| `--through` | Many-to-many: the join model |
| `--nearest <column>` | The children nearest to the parent by their vectors instead of the ones referencing it, `<column>` being the vector of the parent: see [below](#nearest-neighbour). Requires `--postgres --sqlx` |
| `--sort <columns>` | The columns of the child the children can be sorted by, `title,created_at`: see [below](#sort). The fields of the child model by default |
| `--name` | The route segment, `books` by default (`similar` or `nearest_<children>` with `--nearest`) |
| `--sqlx`, `--migration`, `--timestamps`, `--openapi`, `--graphql`, database flags, `--schema`, `--output`, `--force` | As for [`generate-model`]({{ '/cli/generate-model/' | relative_url }}) |

```bash
# one-to-many: GET /project/{id}/books
octopux generate-relation --parent Project --child Book --sqlx --openapi --sqlite

# many-to-many through ProjectCategory: GET /project/{id}/categories
octopux generate-relation --parent Project --child Category --through ProjectCategory --sqlx --openapi --sqlite
```

Declare the file with `mod project_books;` and mount it with `.configure(project_books::configure)`.

## Sort

The route takes `offset` and `limit`, and with `--sqlx`, `sort` and `order`, as the lists of the models: `sort` lists the columns separated by commas, a column prefixed with `-` in descending order, and `order`, `asc` or `desc`, gives the direction of the others.

```bash
# the books of a project, the most recent first, then by title
curl 'localhost:8080/v1/project/1/books?sort=-created_at,title'
```

The columns allowed are in the `SORT_COLUMNS` constant of the relation file, any other one being answered 400 `BAD_REQUEST`: those of `--sort`, or the fields of the child model, but those skipped by serde, the vectors, geometries and JSON values and `deleted_at`. `id` is always allowed, and ends the `ORDER BY` so that the pages neither overlap nor skip rows. With `--graphql`, the field of the relation takes `sort` and `order` too: `{ project(id: 1) { books(sort: "-created_at") { title } } }`.

## Nearest neighbour

With `--nearest`, the relation lists the children nearest to the parent by the cosine distance of their vector to the vector of the parent, the most similar first, with their similarity: `[{ "similarity": 0.92, "recipe": {...} }]`. The child must have been generated with [the semantic search]({{ '/guides/pgvector/' | relative_url }}#semantic-search-generated-by-the-cli): the relation selects the children along the column of its `near` filter, and needs no embedder, the vectors being in the database.

```bash
# the other recipes nearest to a recipe: GET /recipe/{id}/similar
octopux generate-relation --parent Recipe --child Recipe --nearest embedding --postgres --sqlx --openapi --graphql --timestamps

# the wines nearest to a recipe, the two vectors computed by the same model: GET /recipe/{id}/nearest_wines
octopux generate-relation --parent Recipe --child Wine --nearest embedding --postgres --sqlx --openapi --graphql --timestamps
```

The route takes `offset`, `limit`, `min_similarity`, `sort` and `order`, and answers 404 for an unknown parent. `min_similarity` filters before the pagination, so the pages stay full. `sort` takes `similarity` and the columns of the child (given with `--sort`, or read from the child model), `-similarity` by default: `?sort=title` lists the similar recipes by title, `?order=asc` alone the least similar first. The similarity is sorted by the cosine distance, which the HNSW index serves, then by `id`. With `--graphql`, the relation is the `similar` field of the `Recipe` type, and the CLI merges a query of its own into the query root of `src/main.rs`, `similarRecipes(id, offset, limit, minSimilarity, sort, order)` (`recipeNearestWines` for the wines), which answers `ENTITY_NOT_FOUND` for an unknown parent. When the child is the parent, the parent is left out of the query. `--migration` creates the HNSW index of the vector of the child if it does not exist. The other filters of the child, as `other_cuisine` in the recipes example, are added to the query built by `list_related` by hand.
