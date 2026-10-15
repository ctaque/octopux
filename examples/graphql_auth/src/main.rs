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

#![recursion_limit = "512"]

mod auth_extension;
mod page;
mod author;
mod author_books;
mod book;
mod helpers;
use actix_web::{middleware::from_fn, web};
use helpers::AppState;
use octopux::anyhow;
use async_graphql::http::GraphiQLSource;
use async_graphql::{EmptySubscription, MergedObject, Object, Schema};
use async_graphql_actix_web::{GraphQLRequest, GraphQLResponse};
use sqlx::SqlitePool;
mod book_book_pages;
use auth_extension::{require_token, JwtKey, RequireAuth};

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
struct Query(ApiQuery, author::AuthorQuery, book::BookQuery, page::PageQuery);

// The GraphQL mutations, GraphQL refusing a mutation root without fields, `octopux generate-model --graphql` replaces it with:
// `#[derive(MergedObject, Default)] struct Mutation(<model_name>::<Model>Mutation, ...);`
#[derive(MergedObject, Default)]
struct Mutation(author::AuthorMutation, book::BookMutation, page::PageMutation);

type AppSchema = Schema<Query, Mutation, EmptySubscription>;

// Executes the request with the user of its token, `RequireAuth` refusing the private root fields without one
async fn graphql(schema: web::Data<AppSchema>, key: web::Data<JwtKey>, http: actix_web::HttpRequest, req: GraphQLRequest) -> GraphQLResponse {
    let mut req = req.into_inner();
    if let Some(user) = key.authenticate(&http) {
        req = req.data(user);
    }
    schema.execute(req).await.into()
}

async fn graphiql() -> actix_web::HttpResponse {
    actix_web::HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(GraphiQLSource::build().endpoint("/graphql").finish())
}

// The schema, `RequireAuth` refusing its private root fields to anonymous requests. The introspection,
// which GraphiQL reads the documentation from, is disabled unless `introspection` is set, which also
// makes it public so that GraphiQL needs no token
fn schema(state: web::Data<AppState>, introspection: bool) -> AppSchema {
    let mut public = vec![
        ("Query", "apiVersion"), // allow route Query "apiVersion" to bypass authentication
        ("Mutation", "login"), // same for login Mutation
        ("Mutation", "register"), // same for register Mutation
    ];
    if introspection {
        public.extend([("Query", "__schema"), ("Query", "__type")]);
    }
    let builder = Schema::build(Query::default(), Mutation::default(), EmptySubscription)
        .extension(RequireAuth::new(&public))
        // The resolvers of the models read the state from the data of the schema
        .data(state);
    if introspection { builder } else { builder.disable_introspection() }.finish()
}

// The routes, the REST ones requiring a token as the private fields of the schema do
fn configure(cfg: &mut web::ServiceConfig) {
    // Actix does not fall through between scopes sharing a prefix,
    // so resources living under the same scope must be registered together
    cfg.service(
        web::scope("v1") // Where the magic operates
            .wrap(from_fn(require_token))
            .configure(author::configure)
            .configure(author_books::configure)
            .configure(book::configure)
            .configure(page::configure)
            .configure(book_book_pages::configure),
    )
    // GraphQL queries on POST /graphql, GraphiQL on GET /graphql
    .service(
        web::resource("/graphql")
            .route(web::post().to(graphql))
            .route(web::get().to(graphiql)),
    );
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    // The SQLite database is a file created next to the crate, migrated on launch
    let pool = SqlitePool::connect("sqlite://data.db?mode=rwc").await?;
    sqlx::migrate!().run(&pool).await?;
    // One pool shared by every worker
    let state = web::Data::new(AppState { pool });
    // The secret signing the tokens (HS256)
    let jwt_secret = std::env::var("JWT_SECRET").map_err(|_| anyhow::anyhow!("JWT_SECRET must be set"))?;
    let jwt_key = web::Data::new(JwtKey::from_secret(jwt_secret.as_bytes()));
    // The introspection is only enabled with ENVIRONMENT=development, any other value or none disabling it
    let development = std::env::var("ENVIRONMENT").as_deref() == Ok("development");
    let schema = web::Data::new(schema(state.clone(), development));

    actix_web::HttpServer::new(move || {
        actix_web::App::new()
            .configure(configure)
            .app_data(state.clone())
            .app_data(schema.clone())
            .app_data(jwt_key.clone())
    })
    .bind(("127.0.0.1", 8085))?
    .run()
    .await?;
    Ok(())
}

// Exercises the code generated with --graphql: the schema of --bootstrap, the resolvers of
// generate-model and the field of generate-relation, through POST /graphql as a client would
#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{http::header::AUTHORIZATION, http::StatusCode, test};
    use serde_json::{json, Value};
    use sqlx::SqlitePool;

    const SECRET: &[u8] = b"test secret";

    // `#[sqlx::test]` runs the test on a plain tokio runtime, the actix test service spawns its tasks on a LocalSet
    async fn local<F: std::future::Future>(f: F) -> F::Output {
        tokio::task::LocalSet::new().run_until(f).await
    }

    // `#[sqlx::test]` creates a migrated SQLite database per test, dropped afterwards
    async fn app_with(pool: SqlitePool, introspection: bool) -> impl actix_web::dev::Service<actix_http::Request, Response = actix_web::dev::ServiceResponse, Error = actix_web::Error> {
        let state = web::Data::new(AppState { pool });
        test::init_service(
            actix_web::App::new()
                .configure(configure)
                .app_data(web::Data::new(schema(state.clone(), introspection)))
                .app_data(web::Data::new(JwtKey::from_secret(SECRET)))
                .app_data(state),
        )
        .await
    }

    async fn app(pool: SqlitePool) -> impl actix_web::dev::Service<actix_http::Request, Response = actix_web::dev::ServiceResponse, Error = actix_web::Error> {
        app_with(pool, false).await
    }

    // A token of the user 1, valid for an hour
    fn token() -> String {
        let claims = json!({ "sub": "1", "role": "user", "exp": chrono::Utc::now().timestamp() + 3600 });
        jsonwebtoken::encode(&jsonwebtoken::Header::default(), &claims, &jsonwebtoken::EncodingKey::from_secret(SECRET)).unwrap()
    }

    fn bearer() -> (actix_web::http::header::HeaderName, String) {
        (AUTHORIZATION, format!("Bearer {}", token()))
    }

    // The query sent with the token of the user, or anonymously
    async fn send(app: &impl actix_web::dev::Service<actix_http::Request, Response = actix_web::dev::ServiceResponse, Error = actix_web::Error>, query: &str, authenticated: bool) -> Value {
        let mut req = test::TestRequest::post().uri("/graphql").set_json(json!({ "query": query }));
        if authenticated {
            req = req.insert_header(bearer());
        }
        test::call_and_read_body_json(app, req.to_request()).await
    }

    async fn graphql(app: &impl actix_web::dev::Service<actix_http::Request, Response = actix_web::dev::ServiceResponse, Error = actix_web::Error>, query: &str) -> Value {
        send(app, query, true).await
    }

    #[sqlx::test]
    async fn anonymous_requests_only_reach_the_public_fields(pool: SqlitePool) {
        local(async move {
            let app = app(pool).await;
            assert_eq!(send(&app, "{ apiVersion }", false).await, json!({ "data": { "apiVersion": "0.0.0" } }));
            let res = send(&app, "{ authors { id } }", false).await;
            assert_eq!(res["errors"][0]["message"], "Unauthorized", "{}", res);
            let res = send(&app, r#"mutation { createAuthor(input: { name: "Ursula" }) { id } }"#, false).await;
            assert_eq!(res["errors"][0]["message"], "Unauthorized", "{}", res);
        })
        .await
    }

    #[sqlx::test]
    async fn the_rest_routes_require_a_valid_token(pool: SqlitePool) {
        local(async move {
            let app = app(pool).await;
            let anonymous = [
                test::TestRequest::get().uri("/v1/author"),
                test::TestRequest::get().uri("/v1/author/1"),
                test::TestRequest::get().uri("/v1/author/1/books"),
                test::TestRequest::post().uri("/v1/author").set_json(json!({ "name": "Ursula" })),
                test::TestRequest::delete().uri("/v1/author/1"),
                test::TestRequest::get().uri("/v1/author").insert_header((AUTHORIZATION, "Bearer not-a-token")),
            ];
            for req in anonymous {
                let res = test::call_service(&app, req.to_request()).await;
                assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
            }
            let req = test::TestRequest::post().uri("/v1/author").insert_header(bearer()).set_json(json!({ "name": "Ursula" }));
            assert_eq!(test::call_service(&app, req.to_request()).await.status(), StatusCode::OK);
            let req = test::TestRequest::get().uri("/v1/author").insert_header(bearer());
            let authors: Value = test::call_and_read_body_json(&app, req.to_request()).await;
            assert_eq!(authors, json!([{ "id": 1, "name": "Ursula" }]));
        })
        .await
    }

    #[sqlx::test]
    async fn the_introspection_is_only_enabled_in_development(pool: SqlitePool) {
        local(async move {
            let query = "{ __schema { queryType { name } } }";
            let production = app_with(pool.clone(), false).await;
            // a disabled introspection is a private field like the others, resolving to null with a token
            let res = send(&production, query, false).await;
            assert_eq!(res["errors"][0]["message"], "Unauthorized", "{}", res);
            assert_eq!(send(&production, query, true).await, json!({ "data": { "__schema": null } }));
            let development = app_with(pool, true).await;
            assert_eq!(send(&development, query, false).await, json!({ "data": { "__schema": { "queryType": { "name": "Query" } } } }));
            let res = send(&development, r#"{ __type(name: "Query") { name } }"#, false).await;
            assert_eq!(res, json!({ "data": { "__type": { "name": "Query" } } }));
        })
        .await
    }

    #[sqlx::test]
    async fn the_schema_serves_the_api_version_and_graphiql(pool: SqlitePool) {
        local(async move {
            let app = app(pool).await;
            assert_eq!(graphql(&app, "{ apiVersion }").await, json!({ "data": { "apiVersion": "0.0.0" } }));
            let res = test::call_service(&app, test::TestRequest::get().uri("/graphql").to_request()).await;
            assert!(res.status().is_success());
        })
        .await
    }

    #[sqlx::test]
    async fn models_are_created_found_listed_updated_and_deleted(pool: SqlitePool) {
        local(async move {
            let app = app(pool).await;
            let created = graphql(&app, r#"mutation { createAuthor(input: { name: "Ursula" }) { id name } }"#).await;
            assert_eq!(created, json!({ "data": { "createAuthor": { "id": 1, "name": "Ursula" } } }));
            graphql(&app, r#"mutation { createAuthor(input: { name: "Frank" }) { id } }"#).await;

            assert_eq!(graphql(&app, "{ author(id: 1) { name } }").await, json!({ "data": { "author": { "name": "Ursula" } } }));
            assert_eq!(
                graphql(&app, "{ authors(offset: 1, limit: 1) { name } }").await,
                json!({ "data": { "authors": [{ "name": "Frank" }] } })
            );

            let updated = graphql(&app, r#"mutation { updateAuthor(input: { id: 1, name: "Ursula K. Le Guin" }) { id name } }"#).await;
            assert_eq!(updated, json!({ "data": { "updateAuthor": { "id": 1, "name": "Ursula K. Le Guin" } } }));

            let deleted = graphql(&app, "mutation { deleteAuthor(id: 2) { name } }").await;
            assert_eq!(deleted, json!({ "data": { "deleteAuthor": { "name": "Frank" } } }));
            assert_eq!(graphql(&app, "{ authors { id } }").await, json!({ "data": { "authors": [{ "id": 1 }] } }));
        })
        .await
    }

    #[sqlx::test]
    async fn missing_models_are_not_found(pool: SqlitePool) {
        local(async move {
            let app = app(pool).await;
            for query in [
                "{ author(id: 42) { id } }",
                r#"mutation { updateAuthor(input: { id: 42, name: "Nobody" }) { id } }"#,
                "mutation { deleteAuthor(id: 42) { id } }",
            ] {
                let res = graphql(&app, query).await;
                assert_eq!(res["errors"][0]["message"], "ENTITY_NOT_FOUND", "{}", query);
            }
        })
        .await
    }

    #[sqlx::test]
    async fn the_relation_resolves_the_paginated_children_of_the_parent(pool: SqlitePool) {
        local(async move {
            let app = app(pool).await;
            graphql(&app, r#"mutation { createAuthor(input: { name: "Ursula" }) { id } }"#).await;
            graphql(&app, r#"mutation { createAuthor(input: { name: "Frank" }) { id } }"#).await;
            for (title, author_id) in [("The Dispossessed", 1), ("Dune", 2), ("The Lathe of Heaven", 1), ("Earthsea", 1)] {
                graphql(&app, &format!(r#"mutation {{ createBook(input: {{ title: "{}", authorId: {} }}) {{ id }} }}"#, title, author_id)).await;
            }

            assert_eq!(
                graphql(&app, "{ author(id: 1) { name books { title } } }").await,
                json!({ "data": { "author": { "name": "Ursula", "books": [
                    { "title": "The Dispossessed" }, { "title": "The Lathe of Heaven" }, { "title": "Earthsea" }
                ] } } })
            );
            assert_eq!(
                graphql(&app, "{ author(id: 1) { books(offset: 1, limit: 1) { title } } }").await,
                json!({ "data": { "author": { "books": [{ "title": "The Lathe of Heaven" }] } } })
            );
            assert_eq!(
                graphql(&app, "{ authors { name books { title } } }").await["data"]["authors"][1],
                json!({ "name": "Frank", "books": [{ "title": "Dune" }] })
            );

            // the REST routes are still served next to the schema, on the same state
            let req = test::TestRequest::get().uri("/v1/author/1/books").insert_header(bearer()).to_request();
            let books: Value = test::call_and_read_body_json(&app, req).await;
            assert_eq!(books.as_array().unwrap().len(), 3);
        })
        .await
    }
}
