use crate::shared::AppState;
use crate::article::Id;
use serde::{Deserialize, Serialize};
use octopux::{
    HasMany,
    anyhow::Result,
    async_trait,
};
use octopux::pgvector::Vector;
use apistos::ApiComponent;
use schemars::JsonSchema;
use octopux::gen_documented_relation_endpoint;

#[derive(Deserialize, JsonSchema, ApiComponent)]
pub struct ArticleSimilarQuery {
    /// The least similarity of the articles, from -1 to 1 (none by default)
    pub min_similarity: Option<f64>,
    /// Maximum number of rows to return (5 by default, 100 at most)
    pub limit: Option<usize>,
}

/// An article similar to another one, and how similar: the cosine similarity of their embeddings,
/// 1 for the same direction, 0 for unrelated articles
#[derive(Serialize, JsonSchema, ApiComponent, sqlx::FromRow)]
pub struct SimilarArticle {
    pub id: Id,
    pub title: String,
    pub category: String,
    pub similarity: f64,
}

/// Number of similar articles when the query gives no `limit`
const DEFAULT_LIMIT: i64 = 5;
/// Most similar articles a request can ask for
const MAX_LIMIT: i64 = 100;

/// The other articles, the most similar first, served on `GET /article/{id}/similar`
pub struct ArticleSimilar;

#[async_trait]
impl HasMany for ArticleSimilar {
    type Parent = crate::article::Article;
    type Id = Id;
    type Query = ArticleSimilarQuery;
    type Result = Vec<SimilarArticle>;
    type State = AppState;
    const RELATION: &'static str = "similar";

    async fn list_related(id: Id, query: &ArticleSimilarQuery, state: &AppState) -> Result<Option<Vec<SimilarArticle>>> {
        let limit = query.limit.map_or(DEFAULT_LIMIT, |l| (l as i64).min(MAX_LIMIT));
        // The embedding of the article, `None` when it does not exist
        let Some(embedding) = sqlx::query_scalar::<_, Vector>(
            "SELECT embedding FROM article WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&state.pool)
        .await? else {
            return Ok(None);
        };
        // Ordered by the cosine distance, `<=>`, which the HNSW index of the column speeds up;
        // the similarity is 1 minus the distance
        let articles = sqlx::query_as::<_, SimilarArticle>(
            "SELECT id, title, category, 1 - (embedding <=> $2) AS similarity FROM article
             WHERE id <> $1 AND deleted_at IS NULL AND ($3::float8 IS NULL OR 1 - (embedding <=> $2) >= $3)
             ORDER BY embedding <=> $2
             LIMIT $4",
        )
        .bind(id)
        .bind(&embedding)
        .bind(query.min_similarity)
        .bind(limit)
        .fetch_all(&state.pool)
        .await?;
        Ok(Some(articles))
    }
}

// Registers the route of the articles similar to an article, to mount with
// `.configure(article_similar::configure)` in the same scope as the article routes
pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
    gen_documented_relation_endpoint!(ArticleSimilar)(cfg)
}
