#[path = "my_model.rs"]
mod my_model;
#[path = "my_child_model.rs"]
mod my_child_model;
#[path = "helpers.rs"]
mod shared;
mod hooks;

use apistos::app::{BuildConfig, OpenApiWrapper};
use apistos::info::Info;
use apistos::spec::Spec;
use apistos::SwaggerUIConfig;
use async_graphql::http::GraphiQLSource;
use async_graphql::{EmptySubscription, MergedObject, Object, Schema};
use async_graphql_actix_web::GraphQL;
use sqlx::postgres::PgPoolOptions;
use std::default::Default;
use actix_web;
use actix_web::{middleware::Logger};


// The version of the API, so that the query root has a field before the first model is merged
#[derive(Default)]
struct ApiQuery;

#[Object]
impl ApiQuery {
    async fn api_version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
}

// The GraphQL queries, `octopux generate-model --graphql` merges the query root of the model here:
// `struct Query(ApiQuery, <model_name>::<Model>Query, ...);`
#[derive(MergedObject, Default)]
struct Query(ApiQuery, my_model::MyModelQuery, my_child_model::MyChildModelQuery);

// The GraphQL mutations, GraphQL refusing a mutation root without fields, `octopux generate-model --graphql` replaces it with:
// `#[derive(MergedObject, Default)] struct Mutation(<model_name>::<Model>Mutation, ...);`
#[derive(MergedObject, Default)]
struct Mutation(my_model::MyModelMutation, my_child_model::MyChildModelMutation);

async fn graphiql() -> actix_web::HttpResponse {
    actix_web::HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(GraphiQLSource::build().endpoint("/graphql").finish())
}
// The OpenAPI document is served on /openapi.json, and browsable on /swagger
#[actix_web::main]
async fn main() -> anyhow::Result<()>{
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    // The PostgreSQL database is migrated on launch
    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL must be set, e.g. postgres://user:password@localhost:5432/db or sqlite://data.db?mode=rwc"))?;
    let pool = PgPoolOptions::new().connect(&database_url).await?;
    sqlx::migrate!().run(&pool).await?;

    // One pool shared by every worker
    let state = actix_web::web::Data::new(shared::AppState { pool });

    let schema = Schema::build(Query::default(), Mutation::default(), EmptySubscription)
        .data(state.clone())
        .finish();

    actix_web::HttpServer::new(move || {
        let spec = Spec {
            info: Info {
                title: "MyApp API".to_string(),
                version: "1.0.0".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        actix_web::App::new()
            .wrap(Logger::default())
            .document(spec)
            // Actix does not fall through between scopes sharing a prefix,
            // so resources living under the same scope must be registered together
            .service(
                apistos::web::scope("v1")
                    .configure(my_model::configure)
                    .configure(my_child_model::configure)
            )
            .app_data(state.clone())
            .build_with(
                "/openapi.json",
                BuildConfig::default().with(SwaggerUIConfig::new(&"/swagger")),
            )
            // GraphQL queries on POST /graphql, GraphiQL on GET /graphql
            .service(
                actix_web::web::resource("/graphql")
                    .route(actix_web::web::post().to(GraphQL::new(schema.clone())))
                    .route(actix_web::web::get().to(graphiql)),
            )
    })
        .bind(("127.0.0.1", 8085))?
        .run()
        .await?;
    Ok(())
}
