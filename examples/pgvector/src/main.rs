use actix_web::web;
use apistos::app::{BuildConfig, OpenApiWrapper};
use apistos::spec::Spec;
use apistos::SwaggerUIConfig;
use async_graphql::http::GraphiQLSource;
use async_graphql::{EmptySubscription, Schema};
use async_graphql_actix_web::GraphQL;
use sqlx::postgres::PgPoolOptions;
#[path = "helpers.rs"]
mod shared;
mod article;
mod article_similar;

// The GraphQL schema of the articles, the vectors being scalars (octopux `graphql` feature)
type ArticleSchema = Schema<article::ArticleQuery, article::ArticleMutation, EmptySubscription>;

async fn graphiql() -> actix_web::HttpResponse {
    actix_web::HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(GraphiQLSource::build().endpoint("/graphql").finish())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok(); // loads .env if present, never overrides existing vars

    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| std::io::Error::other("DATABASE_URL must be set, e.g. postgres://postgres:postgres@localhost:5433/articles"))?;
    let pool = PgPoolOptions::new()
        .connect(&database_url)
        .await
        .map_err(std::io::Error::other)?;
    // creates the vector extension, the article table, its HNSW index and a few articles
    sqlx::migrate!().run(&pool).await.map_err(std::io::Error::other)?;

    // One pool shared by every worker
    let state = web::Data::new(shared::AppState { pool });
    // The resolvers read the state from the data of the schema
    let schema: ArticleSchema = Schema::build(article::ArticleQuery, article::ArticleMutation, EmptySubscription)
        .data(state.clone())
        .finish();

    actix_web::HttpServer::new(move || {
        actix_web::App::new()
            .document(Spec::default())
            .service(apistos::web::scope("v1")
                .configure(article::configure)
                .configure(article_similar::configure)
            )
            .app_data(state.clone())
            // serves the document on /openapi.json and Swagger UI on /swagger
            .build_with("/openapi.json", BuildConfig::default().with(SwaggerUIConfig::new(&"/swagger")))
            // GraphQL queries on POST /graphql, GraphiQL on GET /graphql
            .service(
                web::resource("/graphql")
                    .route(web::post().to(GraphQL::new(schema.clone())))
                    .route(web::get().to(graphiql)),
            )
    })
        .bind(("127.0.0.1", 8085))?
        .run()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    // The vectors are scalars of the schema, checked without database
    #[test]
    fn the_schema_types_the_vectors_as_scalars() {
        let sdl = Schema::build(article::ArticleQuery, article::ArticleMutation, EmptySubscription).finish().sdl();
        for expected in [
            "scalar Vector",
            "scalar SparseVector",
            "embedding: Vector!",
            "keywords: SparseVector",
            "articles(offset: Int, limit: Int, category: String, near: Vector, keywords: SparseVector): [Article!]!",
            "createArticle(input: NewArticle!): Article!",
        ] {
            assert!(sdl.contains(expected), "`{}` missing from\n{}", expected, sdl);
        }
    }
}
