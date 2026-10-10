// The application state, declared (or re-exported) at the root of the crate
use crate::shared::AppState;
use crate::embedder::recipe_text;
use serde::{Serialize, Deserialize};
use octopux::{
    BeforeSave,
    HttpCreate,
    HttpFindListDelete,
    HttpUpdate,
    SqlxModel,
    SqlxFilter,
    SqlxNewModel,
    SqlxUpdatableModel,
    anyhow::Result,
    async_trait,
    octopux_info,
    gen_documented_endpoint
};
// `Vector` is read from and written to pgvector in its binary format, `[0.12, -0.5, ...]` in JSON
use octopux::pgvector::Vector;
use async_graphql::{InputObject, SimpleObject};
use chrono::{DateTime, Utc};
use apistos::ApiComponent;
use schemars::JsonSchema;

#[derive(Default, Deserialize, JsonSchema, ApiComponent)]
pub struct FindQuery {}
// The filters of the list, and its similarity search: `SqlxFilter` orders the recipes by the
// distance of their embedding to `near`, nearest first, after the conditions. `near` is set by the
// search only, from the embedding of its text: the clients never send a vector
#[derive(Default, Deserialize, JsonSchema, ApiComponent, SqlxFilter)]
#[sqlx_filter(database = "postgres")]
pub struct ListQuery {
    /// Number of rows to skip
    pub offset: Option<usize>,
    /// Maximum number of rows to return (20 by default, 100 at most)
    pub limit: Option<usize>,
    /// The recipes of a cuisine: `italian`, `japanese`, `moroccan`...
    pub cuisine: Option<String>,
    /// The recipes of a course: `starter`, `soup`, `main`, `side`, `dessert`, `breakfast` or `drink`
    pub course: Option<String>,
    /// The vegetarian recipes with `true`, the others with `false`
    pub vegetarian: Option<bool>,
    /// The recipes ready in at most this number of minutes: `minutes <= ...`
    pub minutes_lte: Option<i32>,
    // The recipes nearest to this vector of 384 numbers, by cosine distance:
    // `ORDER BY embedding <=> ...`. Never read from the query string nor documented:
    // `GET /recipe/search?q=...` computes it from a text
    #[sqlx_filter(column = "embedding", op = "nearest")]
    #[serde(skip)]
    #[schemars(skip)]
    pub near: Option<Vector>,
    /// The columns ordering the recipes, after the distance: `minutes`, `-created_at`...
    #[sqlx_filter(sort = "name, cuisine, minutes, created_at")]
    pub sort: Option<String>,
}
#[derive(Deserialize, JsonSchema, ApiComponent)]
pub struct DeleteQuery {}
#[derive(Deserialize, JsonSchema, ApiComponent)]
pub struct SaveQuery {}
#[derive(Deserialize, JsonSchema, ApiComponent)]
pub struct UpdateQuery {}
pub type Id = i64;

// The embedding stays in the database: a recipe is sent without its 384 numbers. In GraphQL, its
// `similar` field is resolved in `graphql.rs`
#[derive(Default, Serialize, Deserialize, JsonSchema, ApiComponent, SimpleObject, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
#[graphql(complex)]
#[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
#[sqlx_model(database = "postgres", timestamps, soft_delete, filter)]
#[octopux_info(path = "recipe")]
pub struct Recipe {
    pub id: Id,
    pub name: String,
    pub cuisine: String,
    pub course: String,
    pub vegetarian: bool,
    pub minutes: i32,
    pub description: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

// The client sends the text of the recipe, `before_save` computes its embedding
#[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
#[http_create(SaveQuery, AppState)]
#[sqlx_model(database = "postgres", model = "Recipe", timestamps, before_save)]
pub struct NewRecipe {
    pub name: String,
    pub cuisine: String,
    pub course: String,
    pub vegetarian: bool,
    pub minutes: i32,
    pub description: String,
    /// Computed by `before_save`, never read from the payload
    #[serde(skip)]
    #[graphql(skip)]
    pub embedding: Option<Vector>,
}

#[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
#[http_update(Id, UpdateQuery, Recipe, FindQuery, AppState)]
#[sqlx_model(database = "postgres", timestamps, soft_delete, before_save)]
pub struct UpdatableRecipe {
    pub id: Id,
    pub name: String,
    pub cuisine: String,
    pub course: String,
    pub vegetarian: bool,
    pub minutes: i32,
    pub description: String,
    /// Computed again by `before_save` from the new text
    #[serde(skip)]
    #[graphql(skip)]
    pub embedding: Option<Vector>,
    pub updated_at: Option<DateTime<Utc>>,
}

// Embeds the recipe before the insert, so that it is found by the searches at once
#[async_trait]
impl BeforeSave<AppState> for NewRecipe {
    async fn before_save(mut self: Self, state: &AppState) -> Result<Self> {
        let text = recipe_text(&self.name, &self.cuisine, &self.course, self.vegetarian, &self.description);
        self.embedding = state.embedder.embed(vec![text]).await?.pop();
        Ok(self)
    }
}

// Embeds the new text of the recipe before the update
#[async_trait]
impl BeforeSave<AppState> for UpdatableRecipe {
    async fn before_save(mut self: Self, state: &AppState) -> Result<Self> {
        let text = recipe_text(&self.name, &self.cuisine, &self.course, self.vegetarian, &self.description);
        self.embedding = state.embedder.embed(vec![text]).await?.pop();
        Ok(self)
    }
}

// Registers the documented routes of the recipe endpoint
// (octopux `openapi` feature), to mount with `.configure(recipe::configure)`
pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
    gen_documented_endpoint!(Recipe, NewRecipe, UpdatableRecipe)(cfg)
}
