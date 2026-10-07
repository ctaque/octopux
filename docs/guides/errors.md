---
title: Errors
parent: Guides
nav_order: 6
---

# Errors

Errors are answered in JSON, with a stable `code` to match on:

```json
{ "code": "ENTITY_NOT_FOUND", "message": "no entity has this id" }
```

| Status | `code` | When |
| --- | --- | --- |
| 400 | `ID_MISMATCH` | The `id` of the `PUT` payload differs from the `id` of the path |
| 400 | `BAD_REQUEST` | Invalid sort, `NOT NULL` or `CHECK` constraint violated, or `Error::BadRequest` |
| 404 | `ENTITY_NOT_FOUND` | No entity has this id (`sqlx::Error::RowNotFound`, or `Error::NotFound`) |
| 409 | `CONFLICT` | Unique or foreign key constraint violated, or `Error::Conflict` |
| 500 | `INTERNAL_ERROR` | Any other error: its message is logged with the [`log`](https://docs.rs/log) crate, not sent |

Return an `octopux::Error` from a model, a relation or a `BeforeSave` hook to answer a client error:

```rust
async fn before_save(mut self: Self, _state: &AppState) -> Result<Self> {
    if !self.email.contains('@') {
        return Err(octopux::Error::BadRequest("the email is invalid".into()).into());
    }
    Ok(self)
}
```

With the `openapi` feature, every error response is documented with the `OctopuxError` schema.
