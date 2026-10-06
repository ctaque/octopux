---
title: GraphQL
parent: Guides
nav_order: 5
---

# GraphQL

Add `--graphql` to the three CLI commands to serve the models on `/graphql` (GraphiQL on `GET /graphql`) :

```bash
octopux --bootstrap --sqlite --graphql
octopux generate-model --name Author --fields --sqlx --migration --graphql --sqlite
octopux generate-model --name Book --fields --sqlx --migration --foreign-keys --graphql --sqlite
octopux generate-relation --parent Author --child Book --foreign-key=author_id --sqlx --migration --graphql --sqlite
```

Each model gets the `author(id)` and `authors(offset, limit)` queries and the `createAuthor`, `updateAuthor` and `deleteAuthor` mutations, merged into the schema of `src/main.rs`. Relations become fields:

```graphql
query {
  authors(limit: 10) {
    id
    name
    books(limit: 5) { id title }
  }
}
```
