use crate::shared::AppState;
use crate::recipe::Id;
use serde::{Deserialize, Serialize};
use octopux::{
    HasMany,
    anyhow::Result,
    async_trait,
};
use octopux::pgvector::Vector;
use apistos::ApiComponent;
use async_graphql::SimpleObject;
use schemars::JsonSchema;
use octopux::gen_documented_relation_endpoint;

#[derive(Deserialize, JsonSchema, ApiComponent)]
pub struct RecipeSimilarQuery {
    /// The least similarity of the recipes, from -1 to 1 (none by default)
    pub min_similarity: Option<f64>,
    /// Only the recipes of another cuisine with `true`: the same dish elsewhere in the world
    pub other_cuisine: Option<bool>,
    /// Maximum number of rows to return (5 by default, 100 at most)
    pub limit: Option<usize>,
}

/// A recipe similar to another one, and how similar: the cosine similarity of their embeddings,
/// 1 for the same text, around 0.5 for unrelated recipes with this model
#[derive(Serialize, JsonSchema, ApiComponent, SimpleObject, sqlx::FromRow)]
pub struct SimilarRecipe {
    pub id: Id,
    pub name: String,
    pub cuisine: String,
    pub course: String,
    pub similarity: f64,
}

/// Number of similar recipes when the query gives no `limit`
const DEFAULT_LIMIT: i64 = 5;
/// Most similar recipes a request can ask for
const MAX_LIMIT: i64 = 100;

/// The other recipes, the most similar first, served on `GET /recipe/{id}/similar`
pub struct RecipeSimilar;

#[async_trait]
impl HasMany for RecipeSimilar {
    type Parent = crate::recipe::Recipe;
    type Id = Id;
    type Query = RecipeSimilarQuery;
    type Result = Vec<SimilarRecipe>;
    type State = AppState;
    const RELATION: &'static str = "similar";

    async fn list_related(id: Id, query: &RecipeSimilarQuery, state: &AppState) -> Result<Option<Vec<SimilarRecipe>>> {
        let limit = query.limit.map_or(DEFAULT_LIMIT, |l| (l as i64).min(MAX_LIMIT));
        // The embedding and the cuisine of the recipe, `None` when it does not exist
        let Some((embedding, cuisine)) = sqlx::query_as::<_, (Vector, String)>(
            "SELECT embedding, cuisine FROM recipe WHERE id = $1 AND deleted_at IS NULL AND embedding IS NOT NULL",
        )
        .bind(id)
        .fetch_optional(&state.pool)
        .await? else {
            return Ok(None);
        };
        // Ordered by the cosine distance, `<=>`, which the HNSW index of the column speeds up;
        // the similarity is 1 minus the distance
        let recipes = sqlx::query_as::<_, SimilarRecipe>(
            "SELECT id, name, cuisine, course, 1 - (embedding <=> $2) AS similarity FROM recipe
             WHERE id <> $1 AND deleted_at IS NULL
               AND ($3::float8 IS NULL OR 1 - (embedding <=> $2) >= $3)
               AND (NOT $4 OR cuisine <> $5)
             ORDER BY embedding <=> $2
             LIMIT $6",
        )
        .bind(id)
        .bind(&embedding)
        .bind(query.min_similarity)
        .bind(query.other_cuisine.unwrap_or(false))
        .bind(cuisine)
        .bind(limit)
        .fetch_all(&state.pool)
        .await?;
        Ok(Some(recipes))
    }
}

// Registers the route of the recipes similar to a recipe, to mount with
// `.configure(recipe_similar::configure)` in the same scope as the recipe routes
pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
    gen_documented_relation_endpoint!(RecipeSimilar)(cfg)
}
