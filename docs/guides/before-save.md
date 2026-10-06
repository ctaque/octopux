---
title: Transforming the payload
parent: Guides
nav_order: 2
---

# Transforming the payload

With `before_save`, implement `BeforeSave`, e.g. to hash a password. An error answers 500:

```rust
#[derive(Serialize, Deserialize, HttpCreate, SqlxNewModel)]
#[http_create(SaveQuery, AppState)]
#[sqlx_model(database = "postgres", model = "User", before_save)]
pub struct NewUser {
    pub email: String,
    pub password: String,
}

#[async_trait]
impl BeforeSave<AppState> for NewUser {
    async fn before_save(mut self: Self, _state: &AppState) -> Result<Self> {
        self.password = hash(&self.password)?;
        Ok(self)
    }
}
```
