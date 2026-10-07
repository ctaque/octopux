//                         -----
//                     -------------
//                   -----  ----------
//                  ---  --------------
//                 ---  ----------------
//                 --- -----------------
//                 --- -----------------
//                 --- -----------------
//                 ---------------------
//       -----      -------------------       -----
//      -------      --  ---------- --      -------
//          ----      ---------------      -----
//           ---      ---------------      ----
//          ----     -----------------     ----
//        ------   ----------------------   ------
//    --------  ---------------------------  --------
//   ------   -------------------------- ----   -------
//  ----    -----  ---------- --- ------- -----    -----
// ----  ------  -------- --- --- ---- ---  ------  ----
// ----        ---- ----  --- ---- ---- -----       ----
//  ----   ------  ----  ---- ----  ----   ------  -----
//  ------      ------   ---- -----  ------      ------
//    ---------------    ----  ----    --------------
//      ----------       ----  ----      ----------
//                       ----  ----
//                 ---   ---- -----   --
//               ------  ---- ----- -------
//              -------  ---- ----- --------
//              ----    ----   -----    ----
//              -----------     -----------
//               ---------        --------
//
// The application state, declared (or re-exported) at the root of the crate
use crate::shared::AppState;
use serde::{Serialize, Deserialize};
use octopux::{
    HttpCreate,
    HttpFindListDelete,
    HttpUpdate,
    SqlxModel,
    SqlxFilter,
    SqlxNewModel,
    SqlxUpdatableModel,
    Model,
    NewModel,
    UpdatableModel,
    octopux_info,
    gen_documented_endpoint
};
// `Vector` and `SparseVector` are read from and written to pgvector in its binary format, and sent
// to the client as JSON: `[0.95, 0.05, 0.1, 0.1]` and `{"dimensions": 8, "indices": [0, 6], "values": [1.0, 1.0]}`,
// in the REST routes as in GraphQL, where they are the `Vector` and `SparseVector` scalars
use octopux::pgvector::{SparseVector, Vector};
use async_graphql::{Context, InputObject, Object, SimpleObject};
use chrono::{DateTime, Utc};
use apistos::ApiComponent;
use schemars::JsonSchema;

#[derive(Default, Deserialize, JsonSchema, ApiComponent)]
pub struct FindQuery {}
// The filters of the list, and its similarity search: `SqlxFilter` orders the articles by the
// distance of their vectors to the vector of the query, nearest first
#[derive(Default, Deserialize, JsonSchema, ApiComponent, SqlxFilter)]
#[sqlx_filter(database = "postgres")]
pub struct ListQuery {
    /// Number of rows to skip
    pub offset: Option<usize>,
    /// Maximum number of rows to return (20 by default, 100 at most)
    pub limit: Option<usize>,
    /// The articles of a category: `category = ...`
    pub category: Option<String>,
    /// The articles nearest to this vector, by cosine distance: `ORDER BY embedding <=> ...`,
    /// `[cooking,sport,tech,travel]`
    #[sqlx_filter(column = "embedding", op = "nearest")]
    // a string in the query string, not an array exploded into `near=...&near=...`
    #[schemars(with = "Option<String>")]
    pub near: Option<Vector>,
    /// The articles nearest to this vector, by Euclidean distance: `ORDER BY embedding <-> ...`
    #[sqlx_filter(column = "embedding", op = "nearest", distance = "l2")]
    #[schemars(with = "Option<String>")]
    pub near_l2: Option<Vector>,
    /// The articles sharing the most weight with these keywords, by inner product:
    /// `ORDER BY keywords <#> ...`, `{1:1,7:1}/8` with the indices from 1
    #[sqlx_filter(column = "keywords", op = "nearest", distance = "inner_product")]
    #[schemars(with = "Option<String>")]
    pub keywords: Option<SparseVector>,
    /// The columns ordering the articles, after the distance: `title`, `-created_at`...
    #[sqlx_filter(sort = "title, category, created_at")]
    pub sort: Option<String>,
}
#[derive(Deserialize, JsonSchema, ApiComponent)]
pub struct DeleteQuery {}
#[derive(Deserialize, JsonSchema, ApiComponent)]
pub struct SaveQuery {}
#[derive(Deserialize, JsonSchema, ApiComponent)]
pub struct UpdateQuery {}
pub type Id = i64;

#[derive(Default, Serialize, Deserialize, JsonSchema, ApiComponent, SimpleObject, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
#[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
#[sqlx_model(database = "postgres", timestamps, soft_delete, filter)]
#[octopux_info(path = "article")]
pub struct Article {
    pub id: Id,
    pub title: String,
    pub category: String,
    /// `vector(4)`, [cooking, sport, tech, travel]
    pub embedding: Vector,
    /// `sparsevec(8)`, the weights of pasta, football, rust, database, japan, mountain, recipe and ai
    pub keywords: Option<SparseVector>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
#[http_create(SaveQuery, AppState)]
#[sqlx_model(database = "postgres", model = "Article", timestamps)]
pub struct NewArticle {
    pub title: String,
    pub category: String,
    pub embedding: Vector,
    pub keywords: Option<SparseVector>,
}

#[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
#[http_update(Id, UpdateQuery, Article, FindQuery, AppState)]
#[sqlx_model(database = "postgres", timestamps, soft_delete)]
pub struct UpdatableArticle {
    pub id: Id,
    pub title: String,
    pub category: String,
    pub embedding: Vector,
    pub keywords: Option<SparseVector>,
    pub updated_at: Option<DateTime<Utc>>,
}

// Registers the documented routes of the article endpoint
// (octopux `openapi` feature), to mount with `.configure(article::configure)`
pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
    gen_documented_endpoint!(Article, NewArticle, UpdatableArticle)(cfg)
}

// GraphQL queries of the article model (async-graphql), the vectors being scalars
#[derive(Default)]
pub struct ArticleQuery;

#[Object]
impl ArticleQuery {
    async fn article(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Article> {
        find(id, app_state(ctx)?).await
    }

    /// A page of articles, filtered and ordered as `GET /v1/article`: `near` orders them by the
    /// cosine distance of their embedding, `keywords` by the inner product of their keywords
    async fn articles(
        &self,
        ctx: &Context<'_>,
        offset: Option<usize>,
        limit: Option<usize>,
        category: Option<String>,
        near: Option<Vector>,
        keywords: Option<SparseVector>,
    ) -> async_graphql::Result<Vec<Article>> {
        let query = ListQuery { offset, limit, category, near, keywords, ..ListQuery::default() };
        Ok(Article::list(&query, app_state(ctx)?).await?)
    }
}

// GraphQL mutations of the article model, the vectors of the inputs being scalars
#[derive(Default)]
pub struct ArticleMutation;

#[Object]
impl ArticleMutation {
    async fn create_article(&self, ctx: &Context<'_>, input: NewArticle) -> async_graphql::Result<Article> {
        Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
    }

    async fn update_article(&self, ctx: &Context<'_>, input: UpdatableArticle) -> async_graphql::Result<Article> {
        let state = app_state(ctx)?;
        let id = input.id;
        find(id, state).await?;
        input.update(&UpdateQuery {}, state).await?;
        find(id, state).await
    }

    async fn delete_article(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Article> {
        let state = app_state(ctx)?;
        let model = find(id, state).await?;
        Ok(model.delete(&DeleteQuery {}, state).await?)
    }
}

// Looks the article up, any error being ENTITY_NOT_FOUND as for the REST routes
async fn find(id: Id, state: &AppState) -> async_graphql::Result<Article> {
    match Article::find(id, &FindQuery::default(), state).await {
        Ok(model) => Ok(*model),
        Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
    }
}

// The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
    Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
}
