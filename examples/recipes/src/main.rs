use actix_web::web;
use apistos::app::{BuildConfig, OpenApiWrapper};
use apistos::spec::Spec;
use apistos::SwaggerUIConfig;
use async_graphql::http::GraphiQLSource;
use async_graphql_actix_web::GraphQL;
use sqlx::postgres::PgPoolOptions;
#[path = "helpers.rs"]
mod shared;
mod embedder;
mod graphql;
mod recipe;
mod recipe_search;
mod recipe_similar;

async fn graphiql() -> actix_web::HttpResponse {
    actix_web::HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(GraphiQLSource::build().endpoint("/graphql").finish())
}

// The search page, compiled into the binary: the frontend sends a text, the backend embeds it
async fn index() -> actix_web::HttpResponse {
    actix_web::HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(include_str!("../static/index.html"))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok(); // loads .env if present, never overrides existing vars

    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| std::io::Error::other("DATABASE_URL must be set, e.g. postgres://postgres:postgres@localhost:5434/recipes"))?;
    let pool = PgPoolOptions::new()
        .connect(&database_url)
        .await
        .map_err(std::io::Error::other)?;
    // creates the vector extension, the recipe table, its HNSW index and 340 recipes
    sqlx::migrate!().run(&pool).await.map_err(std::io::Error::other)?;

    // downloads the model on the first run (about 130 MB), then loads it from `.fastembed_cache`
    let embedder = embedder::Embedder::load().map_err(std::io::Error::other)?;
    // the seeded recipes have no embedding yet: computed once, in a few seconds
    let embedded = embedder::embed_missing(&pool, &embedder).await.map_err(std::io::Error::other)?;
    if embedded > 0 {
        println!("embedded {} recipes", embedded);
    }

    // One pool and one model shared by every worker
    let state = web::Data::new(shared::AppState { pool, embedder });
    // The resolvers read the state from the data of the schema
    let schema = graphql::schema(state.clone());

    println!("listening on http://127.0.0.1:8085, search page on /, Swagger UI on /swagger, GraphiQL on /graphql");
    actix_web::HttpServer::new(move || {
        actix_web::App::new()
            .document(Spec::default())
            .service(apistos::web::scope("v1")
                // before the recipe routes, `/recipe/search` not being a `/recipe/{id}`
                .configure(recipe_search::configure)
                .configure(recipe::configure)
                .configure(recipe_similar::configure)
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
            // the search page, on the same origin as the API
            .route("/", web::get().to(index))
    })
        .bind(("127.0.0.1", 8085))?
        .run()
        .await
}

#[cfg(test)]
mod tests {
    use super::embedder::recipe_text;
    use super::graphql::{RecipeMutation, RecipeQuery};
    use async_graphql::{EmptySubscription, Schema};

    // The schema is checked without database nor model
    #[test]
    fn the_schema_exposes_the_recipes_without_their_embedding() {
        let sdl = Schema::build(RecipeQuery, RecipeMutation, EmptySubscription).finish().sdl();
        for expected in [
            "recipes(offset: Int, limit: Int, cuisine: String, course: String, vegetarian: Boolean, minutesLte: Int, sort: String): [Recipe!]!",
            "searchRecipes(q: String!, offset: Int, limit: Int, cuisine: String, course: String, vegetarian: Boolean, minutesLte: Int): [Recipe!]!",
            "similar(minSimilarity: Float, otherCuisine: Boolean, limit: Int): [SimilarRecipe!]!",
            "createRecipe(input: NewRecipe!): Recipe!",
            "updateRecipe(input: UpdatableRecipe!): Recipe!",
            "deleteRecipe(id: Int!): Recipe!",
        ] {
            assert!(sdl.contains(expected), "`{}` missing from\n{}", expected, sdl);
        }
        assert!(!sdl.contains("embedding:"), "the embedding is exposed in\n{}", sdl);
        assert!(!sdl.contains("Vector"), "a vector is exposed in\n{}", sdl);
    }

    #[test]
    fn the_embedded_text_names_the_cuisine_and_the_course() {
        assert_eq!(
            recipe_text("Miso soup", "japanese", "soup", true, "Light dashi broth."),
            "Miso soup, a vegetarian japanese soup. Light dashi broth."
        );
        assert_eq!(recipe_text("Gyoza", "japanese", "starter", false, "Dumplings."), "Gyoza, a japanese starter. Dumplings.");
    }
}
