use std::fmt;

use actix_web::http::StatusCode;
use actix_web::web::{Data, Json, Query};
use actix_web::{HttpResponse, ResponseError};
use apistos::{api_operation, ApiComponent, ApiErrorComponent};
use octopux::Model;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::recipe::{ListQuery, Recipe};
use crate::shared::AppState;

#[derive(Deserialize, JsonSchema, ApiComponent)]
pub struct SearchQuery {
    /// What you feel like eating, in English: `something warm and comforting for a rainy evening`
    pub q: String,
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
    /// The recipes ready in at most this number of minutes
    pub minutes_lte: Option<i32>,
}

/// An error of the search, answered as the errors of the generated routes: `{"code", "message"}`
#[derive(Debug, ApiErrorComponent)]
#[openapi_error(
    status(code = 400, description = "BAD_REQUEST: the search is empty, or a filter is invalid"),
    status(code = 500, description = "INTERNAL_ERROR: the embedding or the query failed")
)]
pub struct SearchError(octopux::anyhow::Error);

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#}", self.0)
    }
}

impl ResponseError for SearchError {
    fn status_code(&self) -> StatusCode {
        self.0.downcast_ref::<octopux::Error>().map_or(StatusCode::INTERNAL_SERVER_ERROR, |error| error.status_code())
    }

    fn error_response(&self) -> HttpResponse {
        match self.0.downcast_ref::<octopux::Error>() {
            Some(error) => error.error_response(),
            None => {
                eprintln!("search failed: {:#}", self.0);
                HttpResponse::InternalServerError().json(serde_json::json!({"code": "INTERNAL_ERROR", "message": "an internal error occurred"}))
            }
        }
    }
}

impl From<octopux::anyhow::Error> for SearchError {
    fn from(error: octopux::anyhow::Error) -> Self {
        SearchError(error)
    }
}

/// Searches the recipes by meaning
///
/// Computes the embedding of `q` with the model of the recipes, then lists the recipes as
/// `GET /recipe?near=...`: the filters apply, and the recipes nearest to the search come first.
#[api_operation(tag = "recipe", operation_id = "search_recipes")]
pub async fn search(query: Query<SearchQuery>, state: Data<AppState>) -> Result<Json<Vec<Recipe>>, SearchError> {
    Ok(Json(search_recipes(query.into_inner(), &state).await?))
}

/// The search, shared by `GET /recipe/search` and the `searchRecipes` GraphQL query
pub async fn search_recipes(query: SearchQuery, state: &AppState) -> octopux::anyhow::Result<Vec<Recipe>> {
    let SearchQuery { q, offset, limit, cuisine, course, vegetarian, minutes_lte } = query;
    if q.trim().is_empty() {
        return Err(octopux::Error::BadRequest("`q` must tell what to search".into()).into());
    }
    let near = Some(state.embedder.embed_query(&q).await?);
    let list = ListQuery { offset, limit, cuisine, course, vegetarian, minutes_lte, near, sort: None };
    Ok(Recipe::list(&list, state).await?)
}

// Registers `GET /recipe/search`, before the routes of the recipe endpoint so that `search` is
// not taken for the `{id}` of `GET /recipe/{id}`
pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
    cfg.route("/recipe/search", apistos::web::get().to(search));
}
