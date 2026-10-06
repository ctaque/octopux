---
title: Relations
parent: Guides
nav_order: 4
---

# Relations

A relation lists the children of a model on its own paginated route, e.g. `GET /v1/project/{id}/books`. Generate it with [`generate-relation`]({{ '/cli/generate-relation/' | relative_url }}), or implement `HasMany` by hand:

```rust
pub struct ProjectBooks;

#[async_trait]
impl HasMany for ProjectBooks {
    type Parent = Project;
    type Id = Id;
    type Query = ProjectBooksQuery;
    type Result = Vec<Book>;
    type State = AppState;
    const RELATION: &'static str = "books";

    async fn list_related(id: Id, query: &ProjectBooksQuery, state: &AppState) -> Result<Option<Vec<Book>>> {
        // `None` answers 404 when the project does not exist
    }
}
```
