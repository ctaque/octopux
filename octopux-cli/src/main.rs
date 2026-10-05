use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use sqlx::TypeInfo;
use structopt::clap::ArgGroup;
use structopt::StructOpt;
use syn::spanned::Spanned;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write, Error};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

// Whether the messages are colored, enabled by `main` when writing to a terminal and NO_COLOR is not set,
// so the tests and the piped output get plain text
static COLOR: AtomicBool = AtomicBool::new(false);

// Wraps `text` in the ANSI `code` style when colors are enabled,
// the style is restored after the resets of the spans already painted in `text`
fn paint(code: &str, text: &str) -> String {
    if COLOR.load(Ordering::Relaxed) {
        let start = format!("\x1b[{}m", code);
        format!("{}{}\x1b[0m", start, text.replace("\x1b[0m", &format!("\x1b[0m{}", start)))
    } else {
        text.to_string()
    }
}

fn bold(text: &str) -> String { paint("1", text) }
fn dim(text: &str) -> String { paint("2", text) }
fn red(text: &str) -> String { paint("31", text) }
fn green(text: &str) -> String { paint("32", text) }
fn yellow(text: &str) -> String { paint("33", text) }
fn cyan(text: &str) -> String { paint("36", text) }
fn magenta(text: &str) -> String { paint("35", text) }

// Colors the `code` spans of a message, backticks included
fn highlight(message: &str) -> String {
    message
        .split('`')
        .enumerate()
        .map(|(i, part)| if i % 2 == 1 { cyan(&format!("`{}`", part)) } else { part.to_string() })
        .collect()
}

fn success(message: &str) -> String {
    format!("{} {}", green("✔"), highlight(message))
}

fn failure(message: &str) -> String {
    format!("{} {}", red("✘"), highlight(message))
}

fn warning(message: &str) -> String {
    format!("{} {}", yellow("!"), highlight(message))
}

#[derive(Debug, StructOpt)]
#[structopt(name = "octopux", group = ArgGroup::with_name("database"))]
pub struct Cli {
    /// Generates src/main.rs and src/helpers.rs of an actix server, if src/helpers.rs does not exist,
    /// prompting before overwriting an existing src/main.rs, then prompts for adding their dependencies to Cargo.toml with `cargo add`,
    /// without --sqlite, --postgres or --mysql, asks for the database, OpenAPI and GraphQL interactively
    #[structopt(long = "bootstrap")]
    bootstrap: bool,
    /// With --bootstrap, serves the routes on an apistos app documented with OpenAPI and Swagger UI,
    /// to mount models generated with --openapi, instead of a plain actix app
    #[structopt(long = "openapi", requires = "bootstrap")]
    openapi: bool,
    /// With --bootstrap, also serves an async-graphql schema on POST /graphql and GraphiQL on GET /graphql,
    /// to merge the roots of the models generated with --graphql
    #[structopt(long = "graphql", requires = "bootstrap")]
    graphql: bool,
    /// With --bootstrap, connects to a SQLite database in data.db
    #[structopt(long = "sqlite", requires = "bootstrap", group = "database")]
    sqlite: bool,
    /// With --bootstrap, connects to the PostgreSQL database of DATABASE_URL
    #[structopt(long = "postgres", requires = "bootstrap", group = "database")]
    postgres: bool,
    /// With --bootstrap, connects to the MySQL database of DATABASE_URL
    #[structopt(long = "mysql", requires = "bootstrap", group = "database")]
    mysql: bool,
    #[structopt(subcommand)]
    cmd: Option<Opt>,
}

#[derive(Debug, StructOpt)]
pub enum Opt {
    /// Generates a model, requires one of --sqlite, --postgres or --mysql
    #[structopt(name = "generate-model", group = ArgGroup::with_name("database").required(true))]
    GenerateModel {
        #[structopt(short = "n", long = "name")]
        name: String,
        /// Derives JsonSchema and ApiComponent on the model types, for `gen_documented_endpoint!`
        #[structopt(long = "openapi")]
        openapi: bool,
        /// Derives SimpleObject and InputObject on the model types and adds the async-graphql query and mutation
        /// roots of the model, resolved by its Model, NewModel and UpdatableModel functions, requires --fields
        #[structopt(long = "graphql", requires = "fields")]
        graphql: bool,
        /// Interactively prompts for the fields of the model, added to the model, creatable and updatable structs
        #[structopt(long = "fields")]
        fields: bool,
        /// Derives SqlxModel, SqlxNewModel and SqlxUpdatableModel (octopux `sqlx` feature),
        /// which query the `pool` of the AppState, instead of leaving the model functions to fill, requires --fields
        #[structopt(long = "sqlx", requires = "fields")]
        sqlx: bool,
        /// Creates the migration of the model table in the migrations folder next to src, requires --fields
        #[structopt(long = "migration", requires = "fields")]
        migration: bool,
        /// Asks, for each field, for the table and the column it references, among the tables of the DATABASE_URL database
        /// (the tables created by the migrations when it is not set) and the model itself, the migration declares the foreign keys,
        /// requires --migration
        #[structopt(long = "foreign-keys", requires = "migration")]
        foreign_keys: bool,
        /// Asks, for each field, whether its column is unique, the migration declares the unique constraints,
        /// requires --migration
        #[structopt(long = "unique", requires = "migration")]
        unique: bool,
        /// The table of the sqlx queries and the migration, the snake_case model name by default (`book_page` for `BookPage`)
        #[structopt(long = "table", hidden = true)]
        table: Option<String>,
        /// Targets SQLite with the sqlx queries and the migration
        #[structopt(long = "sqlite", group = "database")]
        sqlite: bool,
        /// Targets PostgreSQL with the sqlx queries and the migration
        #[structopt(long = "postgres", group = "database")]
        postgres: bool,
        /// Targets MySQL with the sqlx queries and the migration
        #[structopt(long = "mysql", group = "database")]
        mysql: bool,
        /// Adds `created_at`, `updated_at` and `deleted_at` columns to the model and the migration,
        /// with --sqlx, they are set by the queries, and `delete` becomes a soft delete setting `deleted_at`
        #[structopt(long = "timestamps")]
        timestamps: bool,
        /// Overwrites the model file when it already exists, the code written in it is lost
        #[structopt(long = "force")]
        force: bool,
        /// The folder of the model file, created if missing, src when it exists by default, the working directory otherwise
        #[structopt(long = "output", parse(from_os_str))]
        output: Option<PathBuf>,
    },
    /// Generates a has-many relation, served on `GET /{parent}/{id}/{relation}` and paginated,
    /// requires one of --sqlite, --postgres or --mysql
    #[structopt(name = "generate-relation", group = ArgGroup::with_name("database").required(true))]
    GenerateRelation {
        /// The parent model, e.g. `Project`
        #[structopt(long = "parent")]
        parent: String,
        /// The child model, e.g. `Book`
        #[structopt(long = "child")]
        child: String,
        /// The last segment of the route, the plural of the child by default (`books`)
        #[structopt(long = "name")]
        name: Option<String>,
        /// The column referencing the parent, in the child table or in the --through table,
        /// `{parent}_id` by default (`project_id`)
        #[structopt(long = "foreign-key")]
        foreign_key: Option<String>,
        /// The join model of a many-to-many relation, e.g. `ProjectCategory`
        #[structopt(long = "through")]
        through: Option<String>,
        /// The column of the --through table referencing the child, `{child}_id` by default (`category_id`)
        #[structopt(long = "child-key", requires = "through")]
        child_key: Option<String>,
        /// The table of the parent model, the snake_case parent by default (`project`)
        #[structopt(long = "parent-table", hidden = true)]
        parent_table: Option<String>,
        /// The table of the child model, the snake_case child by default (`book`)
        #[structopt(long = "child-table", hidden = true)]
        child_table: Option<String>,
        /// The join table, the snake_case --through model by default (`project_category`)
        #[structopt(long = "through-table", requires = "through", hidden = true)]
        through_table: Option<String>,
        /// Derives JsonSchema and ApiComponent on the query, and documents the route
        #[structopt(long = "openapi")]
        openapi: bool,
        /// Adds the relation as a paginated field of the GraphQL type of the parent, in the `#[ComplexObject]`
        /// of its model generated with --graphql, the child model must be generated with --graphql too
        #[structopt(long = "graphql")]
        graphql: bool,
        /// Fills the relation with sqlx queries on the `pool` of the AppState
        #[structopt(long = "sqlx")]
        sqlx: bool,
        /// Creates the migration indexing the foreign key in the migrations folder next to src
        #[structopt(long = "migration")]
        migration: bool,
        /// Targets SQLite with the sqlx queries and the migration
        #[structopt(long = "sqlite", group = "database")]
        sqlite: bool,
        /// Targets PostgreSQL with the sqlx queries and the migration
        #[structopt(long = "postgres", group = "database")]
        postgres: bool,
        /// Targets MySQL with the sqlx queries and the migration
        #[structopt(long = "mysql", group = "database")]
        mysql: bool,
        /// The models were generated with --timestamps: the soft deleted rows are skipped
        #[structopt(long = "timestamps")]
        timestamps: bool,
        /// Overwrites the relation file when it already exists, the code written in it is lost
        #[structopt(long = "force")]
        force: bool,
        /// The folder of the relation file, created if missing, src when it exists by default, the working directory otherwise,
        /// with --graphql, the parent model is looked up in it
        #[structopt(long = "output", parse(from_os_str))]
        output: Option<PathBuf>,
    },
    /// Interactively prompts for fields added to the model, creatable and updatable structs of an existing model,
    /// without regenerating it: the code written in the file is kept, requires one of --sqlite, --postgres or --mysql
    #[structopt(name = "add-field", group = ArgGroup::with_name("database").required(true))]
    AddField {
        /// The model the fields are added to, `Book` for the `Book`, `NewBook` and `UpdatableBook` structs
        #[structopt(short = "m", long = "model")]
        model: String,
        /// Creates the migration adding the columns in the migrations folder next to src
        #[structopt(long = "migration")]
        migration: bool,
        /// Asks, for each field, for the table and the column it references, the migration declares the foreign keys,
        /// requires --migration
        #[structopt(long = "foreign-keys", requires = "migration")]
        foreign_keys: bool,
        /// Asks, for each field, whether its column is unique, the migration creates the unique indexes,
        /// requires --migration
        #[structopt(long = "unique", requires = "migration")]
        unique: bool,
        /// SQL default of the added non-optional columns, set on the existing rows (quote the strings: `--default "'none'"`),
        /// asked for each of them otherwise, requires --migration
        #[structopt(long = "default", requires = "migration")]
        default: Option<String>,
        /// The table of the model, the snake_case model name by default (`book_page` for `BookPage`)
        #[structopt(long = "table", hidden = true)]
        table: Option<String>,
        /// Targets SQLite with the migration
        #[structopt(long = "sqlite", group = "database")]
        sqlite: bool,
        /// Targets PostgreSQL with the migration
        #[structopt(long = "postgres", group = "database")]
        postgres: bool,
        /// Targets MySQL with the migration
        #[structopt(long = "mysql", group = "database")]
        mysql: bool,
        /// The folder of the model file, src when it exists by default, the working directory otherwise
        #[structopt(long = "output", parse(from_os_str))]
        output: Option<PathBuf>,
    },
}

const BOOTSTRAP_MAIN: &str = r#"{graphql_attributes}mod helpers;
use actix_web::web;
use helpers::AppState;
use sqlx::{pool};{graphql_imports}
{graphql_roots}
#[actix_web::main]
async fn main() -> std::io::Result<()> {
{pool_connect}
    // sqlx::migrate!().run(&pool).await.unwrap();
    let state = web::Data::new(AppState { pool });{graphql_schema}

    actix_web::HttpServer::new(move || {
        actix_web::App::new()
            // Actix does not fall through between scopes sharing a prefix,
            // so resources living under the same scope must be registered together
            .service(
                web::scope("v1"), // Where the magic operates
                // Mount your models here with `.configure(<model_name>::configure)`
                // after declaring them with `mod <model_name>;`
            )
            .app_data(state.clone()){graphql_routes}
    })
    .bind(("127.0.0.1", 8085))?
    .run()
    .await
}
"#;

const BOOTSTRAP_OPENAPI_MAIN: &str = r#"{graphql_attributes}mod helpers;
use actix_web::web;
use apistos::app::{BuildConfig, OpenApiWrapper};
use apistos::info::Info;
use apistos::spec::Spec;
use apistos::SwaggerUIConfig;
use helpers::AppState;
use sqlx::{pool};{graphql_imports}
{graphql_roots}
#[actix_web::main]
async fn main() -> std::io::Result<()> {
{pool_connect}
    // sqlx::migrate!().run(&pool).await.unwrap();
    let state = web::Data::new(AppState { pool });{graphql_schema}

    actix_web::HttpServer::new(move || {
        let spec = Spec {
            info: Info {
                title: "MyApp REST API".to_string(),
                version: "1.0.0".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        actix_web::App::new()
            .document(spec)
            // Actix does not fall through between scopes sharing a prefix,
            // so resources living under the same scope must be registered together
            .service(
                apistos::web::scope("v1"), // Where the magic operates
                // Mount your models here with `.configure(<model_name>::configure)`
                // after declaring them with `mod <model_name>;`
            )
            .app_data(state.clone())
            .build_with(
                "/openapi.json",
                BuildConfig::default().with(SwaggerUIConfig::new(&"/swagger")),
            ){graphql_routes}
    })
    .bind(("127.0.0.1", 8085))?
    .run()
    .await
}
"#;

// The SQLite database is a file created next to the crate, the servers are reached through DATABASE_URL
const BOOTSTRAP_SQLITE_CONNECT: &str = r#"    let pool = SqlitePool::connect("sqlite://data.db?mode=rwc").await.unwrap();"#;

const BOOTSTRAP_SERVER_CONNECT: &str = r#"    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL not set");
    let pool = {pool}::connect(&database_url).await.unwrap();"#;

// MergedObject nests the roots it merges, those of a few dozen models overflow the default recursion limit
const BOOTSTRAP_GRAPHQL_ATTRIBUTES: &str = "#![recursion_limit = \"512\"]\n\n";

const BOOTSTRAP_GRAPHQL_IMPORTS: &str = r#"
use async_graphql::http::GraphiQLSource;
use async_graphql::{EmptyMutation, EmptySubscription, MergedObject, Object, Schema};
use async_graphql_actix_web::GraphQL;"#;

const BOOTSTRAP_GRAPHQL_ROOTS: &str = r#"
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
struct Query(ApiQuery);

// The GraphQL mutations, GraphQL refusing a mutation root without fields, `octopux generate-model --graphql` replaces it with:
// `#[derive(MergedObject, Default)] struct Mutation(<model_name>::<Model>Mutation, ...);`
type Mutation = EmptyMutation;

async fn graphiql() -> actix_web::HttpResponse {
    actix_web::HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(GraphiQLSource::build().endpoint("/graphql").finish())
}
"#;

const BOOTSTRAP_GRAPHQL_SCHEMA: &str = r#"
    // The resolvers of the models read the state from the data of the schema
    let schema = Schema::build(Query::default(), Mutation::default(), EmptySubscription)
        .data(state.clone())
        .finish();"#;

const BOOTSTRAP_GRAPHQL_ROUTES: &str = r#"
            // GraphQL queries on POST /graphql, GraphiQL on GET /graphql
            .service(
                web::resource("/graphql")
                    .route(web::post().to(GraphQL::new(schema.clone())))
                    .route(web::get().to(graphiql)),
            )"#;

// sqlx pool of the `dialect` database, shared by src/main.rs and src/helpers.rs
fn bootstrap_pool(dialect: Dialect) -> &'static str {
    match dialect {
        Dialect::Sqlite => "SqlitePool",
        Dialect::Postgres => "PgPool",
        Dialect::Mysql => "MySqlPool",
    }
}

// src/main.rs of --bootstrap, on an apistos app with `openapi`, serving the GraphQL schema with `graphql`,
// connected to the `dialect` database
fn render_bootstrap_main(openapi: bool, graphql: bool, dialect: Dialect) -> String {
    let tpl = if openapi { BOOTSTRAP_OPENAPI_MAIN } else { BOOTSTRAP_MAIN };
    let connect = match dialect {
        Dialect::Sqlite => BOOTSTRAP_SQLITE_CONNECT,
        Dialect::Postgres | Dialect::Mysql => BOOTSTRAP_SERVER_CONNECT,
    };
    let [attributes, imports, roots, schema, routes] = if graphql {
        [BOOTSTRAP_GRAPHQL_ATTRIBUTES, BOOTSTRAP_GRAPHQL_IMPORTS, BOOTSTRAP_GRAPHQL_ROOTS, BOOTSTRAP_GRAPHQL_SCHEMA, BOOTSTRAP_GRAPHQL_ROUTES]
    } else {
        ["", "", "", "", ""]
    };
    tpl.replace("{pool_connect}", connect)
        .replace("{pool}", bootstrap_pool(dialect))
        .replace("{graphql_attributes}", attributes)
        .replace("{graphql_imports}", imports)
        .replace("{graphql_roots}", roots)
        .replace("{graphql_schema}", schema)
        .replace("{graphql_routes}", routes)
}

const BOOTSTRAP_HELPERS: &str = r#"use sqlx::{pool};

pub struct AppState {
    pub pool: {pool},
}
"#;

// src/helpers.rs of --bootstrap, the state holding the pool of the `dialect` database
fn render_bootstrap_helpers(dialect: Dialect) -> String {
    BOOTSTRAP_HELPERS.replace("{pool}", bootstrap_pool(dialect))
}

const OPENAPI_IMPORTS: &str = r#"
    use apistos::ApiComponent;
    use schemars::JsonSchema;
    use octopux::gen_documented_endpoint;"#;

const OPENAPI_CONFIGURE: &str = r#"

    // Registers the documented routes of the {entity_lower_case} endpoint
    // (octopux `openapi` feature), to mount with `.configure({entity_lower_case}::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!({entity}, New{entity}, Updatable{entity})(cfg)
    }
"#;

const ENDPOINT_IMPORTS: &str = r#"
    use octopux::gen_endpoint;"#;

const ENDPOINT_CONFIGURE: &str = r#"

    // Registers the routes of the {entity_lower_case} endpoint, to mount with `.configure({entity_lower_case}::configure)`
    pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {
        gen_endpoint!({entity}, New{entity}, Updatable{entity})(cfg)
    }
"#;

const OPENAPI_DERIVES: &str = ", JsonSchema, ApiComponent";

const GRAPHQL_IMPORTS: &str = r#"
    use async_graphql::{ComplexObject, Context, InputObject, Object, SimpleObject};"#;

// Line of the `#[ComplexObject]` of a model generated with --graphql after which
// `generate-relation --graphql` inserts the field of the relation
const GRAPHQL_RELATIONS_MARKER: &str = "        // Relations generated with `octopux generate-relation --graphql`, inserted below";

// The model traits called by the resolvers, implemented by the sqlx derives
const GRAPHQL_SQLX_TRAIT_IMPORTS: &str = "
        Model,
        NewModel,
        UpdatableModel,";

// Query and mutation roots of the model, delegating to the functions of the model traits as the REST routes do,
// `update` and `delete` look the entity up first, so that an unknown id fails as ENTITY_NOT_FOUND
const GRAPHQL_RESOLVERS: &str = r#"

    // Fields of the GraphQL {entity} type resolved by functions, its has-many relations
    #[ComplexObject]
    impl {entity} {
{relations_marker}
    }

    // GraphQL queries of the {entity_lower_case} model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query({module}::{entity}Query, ...);`
    #[derive(Default)]
    pub struct {entity}Query;

    #[Object]
    impl {entity}Query {
        async fn {field}(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<{entity}> {
            find(id, app_state(ctx)?).await
        }

        async fn {plural}(&self, ctx: &Context<'_>{list_args}) -> async_graphql::Result<Vec<{entity}>> {
            Ok({entity}::list(&ListQuery {{list_query}}, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the {entity_lower_case} model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation({module}::{entity}Mutation, ...);`
    #[derive(Default)]
    pub struct {entity}Mutation;

    #[Object]
    impl {entity}Mutation {
        async fn create_{field}(&self, ctx: &Context<'_>, input: New{entity}) -> async_graphql::Result<{entity}> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_{field}(&self, ctx: &Context<'_>, input: Updatable{entity}) -> async_graphql::Result<{entity}> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_{field}(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<{entity}> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the {entity_lower_case} up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<{entity}> {
        match {entity}::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }
"#;

// Columns added by --timestamps, all nullable, `updated_at` is also part of the updatable struct
const TIMESTAMP_COLUMNS: [&str; 3] = ["created_at", "updated_at", "deleted_at"];
const TIMESTAMP_TYPE: &str = "Option<DateTime<Utc>>";
// Type behind the `Id` alias of the generated model
const ID_TYPE: &str = "i64";

#[derive(Debug, Clone, PartialEq)]
struct Field {
    name: String,
    ty: String,
    // foreign key of the column, added to the migration
    references: Option<Reference>,
    // unique constraint of the column, added to the migration
    unique: bool,
    // length of a VARCHAR column, the one of the dialect when None
    length: Option<u32>,
}

// Column referenced by a foreign key
#[derive(Debug, Clone, PartialEq)]
struct Reference {
    table: String,
    column: String,
}

// Table created by a migration, proposed as the target of the foreign keys
#[derive(Debug, Clone, PartialEq)]
struct Table {
    name: String,
    columns: Vec<Column>,
}

#[derive(Debug, Clone, PartialEq)]
struct Column {
    name: String,
    sql_type: String,
    // primary key or unique, which the databases require for a referenced column
    unique: bool,
}

// Types proposed when prompting for a field type, the first one is the default
const FIELD_TYPES: &[&str] = &[
    "String", "i32", "i64", "f64", "bool", "Option<String>", "DateTime<Utc>", "NaiveDateTime", "NaiveDate", "NaiveTime", "Vec<u8>",
];

fn field_types_menu() -> String {
    FIELD_TYPES
        .iter()
        .enumerate()
        .map(|(i, ty)| format!("{} {}", magenta(&format!("{})", i + 1)), ty))
        .collect::<Vec<_>>()
        .join("  ")
}

// Resolves a type answer: empty for the default, a number from the menu, or any custom type,
// a trailing `?` makes it optional (`3?` gives `Option<i64>`, `?` gives `Option<String>`)
fn parse_field_type(answer: &str) -> Option<String> {
    if let Some(inner) = answer.strip_suffix('?') {
        return parse_field_type(inner.trim()).map(|ty| if ty.starts_with("Option<") { ty } else { format!("Option<{}>", ty) });
    }
    if answer.is_empty() {
        return Some(FIELD_TYPES[0].to_string());
    }
    match answer.parse::<usize>() {
        Ok(n) => FIELD_TYPES.get(n.wrapping_sub(1)).map(|ty| ty.to_string()),
        Err(_) => Some(answer.to_string()),
    }
}

// Field names are snake_case: they are also the column names, which PostgreSQL folds to lowercase
// when unquoted, so a `OptStr` field would not find its `optstr` column when decoding the rows
fn is_field_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() || c == '_' => {
            // keywords that cannot be raw identifiers, the other ones are written `r#type`
            chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') && !["_", "self", "super", "crate"].contains(&name)
        }
        _ => false,
    }
}

// Converts a typed field name to snake_case: `optStr`, `OptStr`, `opt-str` and `opt str` all give `opt_str`,
// acronyms stay together (`HTTPCode` gives `http_code`)
fn to_snake_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut snake = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c == '-' || c.is_whitespace() {
            if !snake.is_empty() && !snake.ends_with('_') {
                snake.push('_');
            }
        } else if c.is_uppercase() {
            let prev = if i > 0 { Some(chars[i - 1]) } else { None };
            let next = chars.get(i + 1);
            let boundary = prev.map_or(false, |p| p.is_lowercase() || p.is_ascii_digit())
                || (prev.map_or(false, |p| p.is_uppercase()) && next.map_or(false, |n| n.is_lowercase()));
            if boundary && !snake.ends_with('_') {
                snake.push('_');
            }
            snake.extend(c.to_lowercase());
        } else {
            snake.push(c);
        }
    }
    snake
}

// Asks `message` after a blank line, which spaces the prompts out
fn prompt<R: BufRead, W: Write>(input: &mut R, output: &mut W, message: &str) -> Result<Option<String>, Error> {
    write!(output, "\n{}", message)?;
    output.flush()?;
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    Ok(Some(line.trim().to_string()))
}

// Asks for field names and types until an empty name (or end of input) is entered,
// `timestamps` reserves the `created_at`, `updated_at` and `deleted_at` names,
// with a `dialect`, only accepts types with a column type in its database (see `Dialect::sql_types`),
// with `tables` (the model table and the tables of the migrations), asks for the column each field references,
// with `unique`, asks whether the column of each field is unique
fn read_fields<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    timestamps: bool,
    dialect: Option<Dialect>,
    tables: Option<(&str, &[Table])>,
    unique: bool,
) -> Result<Vec<Field>, Error> {
    read_new_fields(input, output, timestamps, &[], dialect, tables, unique)
}

// `read_fields` of a model already declaring the fields `declared`, whose names are refused
fn read_new_fields<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    timestamps: bool,
    declared: &[String],
    dialect: Option<Dialect>,
    tables: Option<(&str, &[Table])>,
    unique: bool,
) -> Result<Vec<Field>, Error> {
    let mut fields: Vec<Field> = Vec::new();
    let mut reserved: Vec<&str> = if timestamps { vec!["id", "created_at", "updated_at", "deleted_at"] } else { vec!["id"] };
    reserved.extend(declared.iter().map(String::as_str));
    writeln!(output, "{}", bold("Model fields"))?;
    if !declared.is_empty() {
        writeln!(output, "{}", highlight(&format!("Enter the fields to add (empty name to finish), the model already declares {}", declared.join(", "))))?;
    } else if timestamps {
        writeln!(output, "{}", highlight(&format!("Enter the model fields (empty name to finish), `id: Id` ({}), `created_at`, `updated_at` and `deleted_at` are already declared", ID_TYPE)))?;
    } else {
        writeln!(output, "{}", highlight(&format!("Enter the model fields (empty name to finish), `id: Id` ({}) is already declared", ID_TYPE)))?;
    }
    writeln!(output, "{}", highlight("Wrap a type in `Option<T>` (e.g. `Option<i32>`) to make the field optional, its column is then nullable"))?;
    writeln!(output, "{}", dim(&highlight("Tips: `name:type` skips the type question (`stars:i32`, `stars:2`), a trailing `?` makes the type optional (`2?`), `-` removes the last field")))?;
    loop {
        let message = format!("{} {} ", cyan("?"), bold(&format!("Field {} name ›", fields.len() + 1)));
        let answer = match prompt(input, output, &message)? {
            Some(answer) if !answer.is_empty() => answer,
            _ => break,
        };
        if answer == "-" {
            match fields.pop() {
                Some(field) => writeln!(output, "{}", warning(&format!("Field `{}` removed", field.name)))?,
                None => writeln!(output, "{}", warning("No field to remove"))?,
            }
            continue;
        }
        // `name:type` gives the type with the name
        let (name, inline_type) = match answer.split_once(':') {
            Some((name, ty)) => (name.trim().to_string(), Some(ty.trim().to_string())),
            None => (answer, None),
        };
        let snake = to_snake_case(&name);
        if !is_field_name(&snake) {
            writeln!(output, "{}", failure(&format!("`{}` is not a valid field name, use snake_case", name)))?;
            continue;
        }
        if snake != name {
            writeln!(output, "  {}", dim(&highlight(&format!("`{}` renamed to `{}`", name, snake))))?;
        }
        let name = snake;
        if reserved.contains(&name.as_str()) || fields.iter().any(|f| f.name == name) {
            writeln!(output, "{}", failure(&format!("Field `{}` is already declared", name)))?;
            continue;
        }
        let mut answer = inline_type;
        if answer.is_none() {
            writeln!(output, "  {}", field_types_menu())?;
        }
        let ty = loop {
            let answer = match answer.take() {
                Some(answer) => answer,
                None => {
                    let message = format!(
                        "{} {} {} ",
                        cyan("?"),
                        bold(&format!("Type of {} ›", cyan(&format!("`{}`", name)))),
                        dim(&format!("(number or custom type) [{}]", FIELD_TYPES[0]))
                    );
                    prompt(input, output, &message)?.unwrap_or_default()
                }
            };
            match check_field_type(&answer, dialect) {
                Ok(ty) => break ty,
                Err(error) => {
                    writeln!(output, "{}", failure(&error))?;
                    writeln!(output, "  {}", field_types_menu())?;
                }
            }
        };
        let length = match dialect.and_then(|d| d.sql_column_type(&ty)).and_then(|(sql, _)| varchar_length(sql)) {
            // None for the length of the dialect
            Some(default) => Some(read_length(input, output, &name, dialect.unwrap_or(Dialect::Sqlite), default)?).filter(|l| *l != default),
            None => None,
        };
        let column = dialect.and_then(|d| d.column_type(&ty, length));
        let references = match tables {
            Some((own, existing)) => {
                // the model can reference itself (a `parent_id`), with the fields declared so far
                let mut candidates: Vec<Table> = existing.iter().filter(|t| t.name != own).cloned().collect();
                candidates.push(model_table(own, &fields, dialect));
                read_reference(input, output, &name, column.as_deref(), dialect, own, &candidates)?
            }
            None => None,
        };
        let unique = unique && read_unique(input, output, &name, column.as_deref(), dialect)?;
        let field = Field { name, ty, references, unique, length };
        writeln!(output, "  {} {}", green("✔"), field_line(&field, 0))?;
        fields.push(field);
    }
    if !fields.is_empty() {
        writeln!(output, "{}", fields_summary(&fields, timestamps))?;
    }
    Ok(fields)
}

// The length of a `VARCHAR(n)` column type
fn varchar_length(sql: &str) -> Option<u32> {
    sql.strip_prefix("VARCHAR(")?.strip_suffix(')')?.parse().ok()
}

// Asks for the length of the VARCHAR column of the field `name`, `default` when empty
fn read_length<R: BufRead, W: Write>(input: &mut R, output: &mut W, name: &str, dialect: Dialect, default: u32) -> Result<u32, Error> {
    let max = dialect.max_varchar_length();
    loop {
        let message = format!(
            "{} {} {} ",
            cyan("?"),
            bold(&format!("Length of {} ›", cyan(&format!("`{}`", name)))),
            dim(&format!("(VARCHAR, 1 to {}) [{}]", max, default))
        );
        let answer = match prompt(input, output, &message)? {
            Some(answer) if !answer.is_empty() => answer,
            _ => return Ok(default),
        };
        match answer.parse::<u32>() {
            Ok(length) if (1..=max).contains(&length) => return Ok(length),
            _ => writeln!(output, "{}", failure(&format!("`{}` is not a {} VARCHAR length, pick 1 to {}", answer, dialect.name(), max)))?,
        }
    }
}

// Asks whether the `column` of the field `name` is unique, no by default,
// warns when MySQL refuses a unique index on its column type
fn read_unique<R: BufRead, W: Write>(input: &mut R, output: &mut W, name: &str, column: Option<&str>, dialect: Option<Dialect>) -> Result<bool, Error> {
    let message = format!("{} {} {} ", cyan("?"), bold(&format!("Is {} unique ›", cyan(&format!("`{}`", name)))), dim("(y/N)"));
    let answer = prompt(input, output, &message)?.unwrap_or_default().to_lowercase();
    let unique = matches!(answer.as_str(), "y" | "yes");
    if let (true, Some(Dialect::Mysql), Some(sql)) = (unique, dialect, column) {
        if sql.ends_with("BLOB") || sql.ends_with("TEXT") {
            writeln!(output, "{}", warning(&format!("MySQL refuses a unique index on the {} column `{}` without a key length, `-` removes the field", sql, name)))?;
        }
    }
    Ok(unique)
}

// The type of a type answer (see `parse_field_type`), or why it is refused,
// with a `dialect`, only accepts types with a column type in its database
fn check_field_type(answer: &str, dialect: Option<Dialect>) -> Result<String, String> {
    match (parse_field_type(answer), dialect) {
        (Some(ty), Some(dialect)) if dialect.sql_column_type(&ty).is_none() => Err(format!(
            "`{}` has no {} column type, use one of {}, or Option<T> of them",
            ty,
            dialect.name(),
            dialect.sql_types().iter().map(|(ty, _)| *ty).collect::<Vec<_>>().join(", ")
        )),
        (Some(ty), _) => Ok(ty),
        (None, _) => Err(format!("`{}` is not in the list, pick 1 to {}", answer, FIELD_TYPES.len())),
    }
}

// `name: Type`, the name padded to `width`, optional types flagged as nullable and unique columns as unique, followed by the referenced column
fn field_line(field: &Field, width: usize) -> String {
    let nullable = if field.ty.starts_with("Option<") { dim(" (nullable)") } else { String::new() };
    let unique = if field.unique { dim(" (unique)") } else { String::new() };
    let length = field.length.map_or(String::new(), |l| dim(&format!(" (length {})", l)));
    let references = match &field.references {
        Some(r) => format!(" {} {}", dim("→"), magenta(&format!("{} ({})", r.table, r.column))),
        None => String::new(),
    };
    format!("{}: {}{}{}{}{}", bold(&format!("{:<width$}", field.name, width = width)), yellow(&field.ty), length, nullable, unique, references)
}

// The table of the model being generated, with its `id` and the fields declared so far
fn model_table(name: &str, fields: &[Field], dialect: Option<Dialect>) -> Table {
    let sql_type = |f: &Field| dialect.and_then(|d| d.column_type(&f.ty, f.length)).unwrap_or_default();
    let id_type = dialect.map_or(String::new(), |d| d.id_column().split_whitespace().nth(1).unwrap_or_default().to_string());
    let id = Column { name: "id".to_string(), sql_type: id_type, unique: true };
    let columns = fields.iter().map(|f| Column { name: f.name.clone(), sql_type: sql_type(f), unique: f.unique });
    Table { name: name.to_string(), columns: std::iter::once(id).chain(columns).collect() }
}

// The item of `items` picked by its number in the menu or by its name
fn pick<'a, T>(items: &'a [T], answer: &str, name: impl Fn(&T) -> &str) -> Option<&'a T> {
    match answer.parse::<usize>() {
        Ok(n) => items.get(n.wrapping_sub(1)),
        Err(_) => items.iter().find(|item| name(item).eq_ignore_ascii_case(answer)),
    }
}

fn menu<T>(items: &[T], label: impl Fn(&T) -> String) -> String {
    items
        .iter()
        .enumerate()
        .map(|(i, item)| format!("{} {}", magenta(&format!("{})", i + 1)), label(item)))
        .collect::<Vec<_>>()
        .join("  ")
}

// Asks for the table and the column referenced by the field `name` of column type `sql`, None when it references nothing,
// warns when the column is not unique or not of the type of the field, which the databases refuse
fn read_reference<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    name: &str,
    sql: Option<&str>,
    dialect: Option<Dialect>,
    own: &str,
    tables: &[Table],
) -> Result<Option<Reference>, Error> {
    let tables_menu = menu(tables, |t| if t.name == own { format!("{} {}", t.name, dim("(this model)")) } else { t.name.clone() });
    writeln!(output, "  {}", tables_menu)?;
    let table = loop {
        let message = format!(
            "{} {} {} ",
            cyan("?"),
            bold(&format!("Table referenced by {} ›", cyan(&format!("`{}`", name)))),
            dim("(number or name, empty for none)")
        );
        let answer = prompt(input, output, &message)?.unwrap_or_default();
        if answer.is_empty() {
            return Ok(None);
        }
        match pick(tables, &answer, |t| &t.name) {
            Some(table) => break table,
            None => {
                writeln!(output, "{}", failure(&format!("`{}` is not a known table, pick 1 to {}", answer, tables.len())))?;
                writeln!(output, "  {}", tables_menu)?;
            }
        }
    };
    // the primary key by default
    let Some(default) = table.columns.iter().find(|c| c.unique).or(table.columns.first()) else {
        writeln!(output, "{}", warning(&format!("Table `{}` has no column, `{}` references nothing", table.name, name)))?;
        return Ok(None);
    };
    let columns_menu = menu(&table.columns, |c| {
        let key = if c.unique { dim(" (unique)") } else { String::new() };
        format!("{} {}{}", c.name, dim(&c.sql_type), key)
    });
    writeln!(output, "  {}", columns_menu)?;
    let column = loop {
        let message = format!(
            "{} {} {} ",
            cyan("?"),
            bold(&format!("Column of {} referenced by {} ›", cyan(&format!("`{}`", table.name)), cyan(&format!("`{}`", name)))),
            dim(&format!("(number or name) [{}]", default.name))
        );
        let answer = prompt(input, output, &message)?.unwrap_or_default();
        if answer.is_empty() {
            break default;
        }
        match pick(&table.columns, &answer, |c| &c.name) {
            Some(column) => break column,
            None => {
                writeln!(output, "{}", failure(&format!("`{}` is not a column of `{}`, pick 1 to {}", answer, table.name, table.columns.len())))?;
                writeln!(output, "  {}", columns_menu)?;
            }
        }
    };
    let target = format!("{}.{}", table.name, column.name);
    if !column.unique {
        writeln!(output, "{}", warning(&format!("`{}` is neither a primary key nor unique, the database refuses the foreign key without a unique index on it", target)))?;
    }
    if let (Some(dialect), Some(sql)) = (dialect, sql) {
        if !dialect.same_column_type(sql, &column.sql_type) {
            writeln!(
                output,
                "{}",
                warning(&format!("`{}` is {} and `{}` is {}, the foreign key may be refused, `-` removes the field", name, sql, target, column.sql_type))
            )?;
        }
    }
    Ok(Some(Reference { table: table.name.clone(), column: column.name.clone() }))
}

// Removes the comments, the quotes and the schema of an SQL name: `"public"."author"` gives `author`
fn sql_name(name: &str) -> String {
    let name = name.rsplit('.').next().unwrap_or(name);
    name.trim_matches(|c| matches!(c, '"' | '`' | '[' | ']')).to_string()
}

// Splits on the commas outside parentheses, `DECIMAL(10, 2)` stays whole
fn split_top_level(body: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut depth, mut start) = (0, 0);
    for (i, c) in body.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(body[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(body[start..].trim());
    parts.into_iter().filter(|p| !p.is_empty()).collect()
}

// Keywords ending the type of a column definition
const COLUMN_CONSTRAINTS: &[&str] = &[
    "NOT", "NULL", "PRIMARY", "REFERENCES", "DEFAULT", "UNIQUE", "AUTO_INCREMENT", "AUTOINCREMENT", "CHECK", "CONSTRAINT", "GENERATED", "COLLATE",
];

// First keywords of the table constraints
const TABLE_CONSTRAINTS: &[&str] = &["CONSTRAINT", "PRIMARY", "UNIQUE", "FOREIGN", "CHECK", "KEY", "INDEX"];

// The column a single column PRIMARY KEY or UNIQUE table constraint makes unique
fn unique_key(constraint: &str) -> Option<String> {
    let upper = constraint.to_ascii_uppercase();
    let unique = (upper.contains("PRIMARY KEY") || upper.contains("UNIQUE")) && !upper.contains("FOREIGN KEY");
    let (open, close) = (constraint.find('(')?, constraint.find(')')?);
    match constraint[open + 1..close].split(',').collect::<Vec<_>>().as_slice() {
        [key] if unique => Some(sql_name(key.trim())),
        _ => None,
    }
}

// Columns of a CREATE TABLE body, the single column PRIMARY KEY and UNIQUE table constraints mark their column unique
fn parse_columns(body: &str) -> Vec<Column> {
    let mut columns = Vec::new();
    let mut unique_keys = Vec::new();
    for definition in split_top_level(body) {
        let upper = definition.to_ascii_uppercase();
        let words: Vec<&str> = definition.split_whitespace().collect();
        let first = upper.split_whitespace().next().unwrap_or_default();
        if TABLE_CONSTRAINTS.contains(&first) {
            unique_keys.extend(unique_key(definition));
            continue;
        }
        let sql_type: Vec<&str> = words[1..]
            .iter()
            .take_while(|w| !COLUMN_CONSTRAINTS.contains(&w.to_ascii_uppercase().as_str()))
            .copied()
            .collect();
        let unique = upper.contains("PRIMARY KEY") || upper.split_whitespace().any(|w| w == "UNIQUE");
        columns.push(Column { name: sql_name(words[0]), sql_type: sql_type.join(" "), unique });
    }
    for column in columns.iter_mut() {
        column.unique |= unique_keys.contains(&column.name);
    }
    columns
}

// Tables created by the CREATE TABLE statements of a migration
fn parse_tables(sql: &str) -> Vec<Table> {
    let sql = sql.lines().map(|line| line.split("--").next().unwrap_or_default()).collect::<Vec<_>>().join("\n");
    // same byte offsets as `sql`
    let upper = sql.to_ascii_uppercase();
    let mut tables = Vec::new();
    let mut rest = 0;
    while let Some(start) = upper[rest..].find("CREATE TABLE") {
        let mut pos = rest + start + "CREATE TABLE".len();
        pos += upper[pos..].len() - upper[pos..].trim_start().len();
        if upper[pos..].starts_with("IF NOT EXISTS") {
            pos += "IF NOT EXISTS".len();
        }
        let Some(open) = sql[pos..].find('(').map(|i| pos + i) else { break };
        let mut depth = 0;
        let close = sql[open..].char_indices().find_map(|(i, c)| {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            (depth == 0).then_some(open + i)
        });
        let Some(close) = close else { break };
        tables.push(Table { name: sql_name(sql[pos..open].trim()), columns: parse_columns(&sql[open + 1..close]) });
        rest = close;
    }
    tables
}

// Tables created by the migrations of `dir`, in the order of the migrations, with the columns added, dropped and renamed
// and the unique indexes created by the following statements,
// a table created by several migrations keeps its first definition, which `IF NOT EXISTS` applies
fn migration_tables(dir: &Path) -> Vec<Table> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().map_or(false, |e| e == "sql") && !path.to_string_lossy().ends_with(".down.sql"))
        .collect();
    paths.sort();
    let mut tables: Vec<Table> = Vec::new();
    for path in paths {
        let Ok(sql) = fs::read_to_string(&path) else { continue };
        for statement in sql_statements(&sql) {
            apply_statement(&mut tables, &statement);
        }
    }
    tables
}

// The statements of a migration, without their comments, split on the semicolons outside the quotes
fn sql_statements(sql: &str) -> Vec<String> {
    let sql = sql.lines().map(|line| line.split("--").next().unwrap_or_default()).collect::<Vec<_>>().join("\n");
    let mut statements = Vec::new();
    let (mut quote, mut start) = (None, 0);
    for (i, c) in sql.char_indices() {
        match (c, quote) {
            ('\'' | '"' | '`', None) => quote = Some(c),
            (c, Some(q)) if c == q => quote = None,
            (';', None) => {
                statements.push(sql[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    statements.push(sql[start..].trim().to_string());
    statements.into_iter().filter(|s| !s.is_empty()).collect()
}

// The words of `sql` after the leading keywords of `skipped` it starts with (`IF NOT EXISTS`...), case insensitive
fn skip_keywords<'a>(sql: &'a str, skipped: &[&str]) -> &'a str {
    let mut rest = sql.trim_start();
    'next: loop {
        for keywords in skipped {
            let words: Vec<&str> = rest.splitn(keywords.split_whitespace().count() + 1, char::is_whitespace).collect();
            let matches = keywords.split_whitespace().zip(&words).all(|(k, w)| w.eq_ignore_ascii_case(k));
            if matches && words.len() > keywords.split_whitespace().count() {
                rest = words.last().unwrap_or(&"").trim_start();
                continue 'next;
            }
        }
        return rest;
    }
}

// The first word of `sql` and the rest, an SQL name ending at a parenthesis too (`book(isbn)`)
fn first_word(sql: &str) -> (&str, &str) {
    let sql = sql.trim_start();
    let end = sql.find(|c: char| c.is_whitespace() || c == '(').unwrap_or(sql.len());
    (&sql[..end], sql[end..].trim_start())
}

// Applies a CREATE TABLE, ALTER TABLE, CREATE UNIQUE INDEX or DROP TABLE statement to the tables of the migrations,
// the other statements are ignored
fn apply_statement(tables: &mut Vec<Table>, statement: &str) {
    let words: Vec<String> = statement.split_whitespace().take(3).map(str::to_ascii_uppercase).collect();
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["CREATE", "TABLE", ..] => {
            for table in parse_tables(statement) {
                if !tables.iter().any(|t| t.name == table.name) {
                    tables.push(table);
                }
            }
        }
        ["DROP", "TABLE", ..] => {
            let names = skip_keywords(&statement[statement.to_ascii_uppercase().find("TABLE").unwrap_or(0) + 5..], &["IF EXISTS"]);
            for name in names.split(',') {
                let name = sql_name(first_word(name).0);
                tables.retain(|t| t.name != name);
            }
        }
        ["ALTER", "TABLE", ..] => {
            let rest = skip_keywords(&statement[statement.to_ascii_uppercase().find("TABLE").unwrap_or(0) + 5..], &["IF EXISTS", "ONLY"]);
            let (name, actions) = first_word(rest);
            let name = sql_name(name);
            let Some(table) = tables.iter_mut().find(|t| t.name == name) else { return };
            for action in split_top_level(actions) {
                apply_alter_action(table, action);
            }
        }
        ["CREATE", "UNIQUE", "INDEX"] => {
            let upper = statement.to_ascii_uppercase();
            let Some(on) = upper.find(" ON ") else { return };
            let (name, columns) = first_word(skip_keywords(&statement[on + 4..], &["ONLY"]));
            let name = sql_name(name);
            if let (Some(table), Some(column)) = (tables.iter_mut().find(|t| t.name == name), unique_key(&format!("UNIQUE {}", columns))) {
                for c in table.columns.iter_mut().filter(|c| c.name == column) {
                    c.unique = true;
                }
            }
        }
        _ => {}
    }
}

// Applies an action of an ALTER TABLE statement: ADD, DROP or RENAME a column, ADD a unique constraint, RENAME the table
fn apply_alter_action(table: &mut Table, action: &str) {
    let (keyword, rest) = first_word(action);
    match keyword.to_ascii_uppercase().as_str() {
        "ADD" => {
            let definition = skip_keywords(rest, &["COLUMN", "IF NOT EXISTS"]);
            let first = first_word(definition).0.to_ascii_uppercase();
            if TABLE_CONSTRAINTS.contains(&first.as_str()) {
                if let Some(key) = unique_key(definition) {
                    for c in table.columns.iter_mut().filter(|c| c.name == key) {
                        c.unique = true;
                    }
                }
            } else {
                for column in parse_columns(definition) {
                    if !table.columns.iter().any(|c| c.name == column.name) {
                        table.columns.push(column);
                    }
                }
            }
        }
        "DROP" => {
            let (name, _) = first_word(skip_keywords(rest, &["COLUMN", "IF EXISTS"]));
            if !TABLE_CONSTRAINTS.contains(&name.to_ascii_uppercase().as_str()) {
                let name = sql_name(name);
                table.columns.retain(|c| c.name != name);
            }
        }
        "RENAME" => {
            let (word, after) = first_word(rest);
            match word.to_ascii_uppercase().as_str() {
                "TO" | "AS" => table.name = sql_name(first_word(after).0),
                "INDEX" | "KEY" | "CONSTRAINT" => {}
                _ => {
                    let (from, after) = first_word(skip_keywords(rest, &["COLUMN"]));
                    let (to, _) = first_word(skip_keywords(after, &["TO"]));
                    let (from, to) = (sql_name(from), sql_name(to));
                    for c in table.columns.iter_mut().filter(|c| c.name == from) {
                        c.name = to.clone();
                    }
                }
            }
        }
        _ => {}
    }
}

// Database of a connection url, from its scheme
fn url_dialect(url: &str) -> Option<Dialect> {
    match url.split(':').next()? {
        "sqlite" => Some(Dialect::Sqlite),
        "postgres" | "postgresql" => Some(Dialect::Postgres),
        "mysql" | "mariadb" => Some(Dialect::Mysql),
        _ => None,
    }
}

// Table of the sqlx migrations, not a model to reference
const SQLX_MIGRATIONS_TABLE: &str = "_sqlx_migrations";

// Columns of the current schema, with their udt name (`int8`, `_text` for an array of text)
// and whether a single column primary key or unique constraint covers them
const POSTGRES_COLUMNS: &str = "SELECT c.table_name::text, c.column_name::text, c.udt_name::text,
    EXISTS (
        SELECT 1 FROM information_schema.table_constraints tc
        JOIN information_schema.key_column_usage k
            ON k.constraint_schema = tc.constraint_schema AND k.constraint_name = tc.constraint_name
        WHERE tc.table_schema = c.table_schema AND tc.table_name = c.table_name AND k.column_name = c.column_name
            AND tc.constraint_type IN ('PRIMARY KEY', 'UNIQUE')
            AND (SELECT count(*) FROM information_schema.key_column_usage k2
                 WHERE k2.constraint_schema = tc.constraint_schema AND k2.constraint_name = tc.constraint_name) = 1
    )
FROM information_schema.columns c
JOIN information_schema.tables t ON t.table_schema = c.table_schema AND t.table_name = c.table_name
WHERE c.table_schema = current_schema() AND t.table_type = 'BASE TABLE' AND c.table_name <> '_sqlx_migrations'
ORDER BY c.table_name, c.ordinal_position";

// information_schema columns are cast, MySQL 8 returns some of them as binary strings
const MYSQL_COLUMNS: &str = "SELECT CAST(c.TABLE_NAME AS CHAR), CAST(c.COLUMN_NAME AS CHAR), CAST(c.COLUMN_TYPE AS CHAR),
    CAST(c.COLUMN_KEY IN ('PRI', 'UNI') AS SIGNED)
FROM information_schema.COLUMNS c
JOIN information_schema.TABLES t ON t.TABLE_SCHEMA = c.TABLE_SCHEMA AND t.TABLE_NAME = c.TABLE_NAME
WHERE c.TABLE_SCHEMA = DATABASE() AND t.TABLE_TYPE = 'BASE TABLE' AND c.TABLE_NAME <> '_sqlx_migrations'
ORDER BY c.TABLE_NAME, c.ORDINAL_POSITION";

// Groups the (table, column) rows, ordered by table, into tables
fn group_columns(rows: impl IntoIterator<Item = (String, Column)>) -> Vec<Table> {
    let mut tables: Vec<Table> = Vec::new();
    for (table, column) in rows {
        match tables.last_mut() {
            Some(last) if last.name == table => last.columns.push(column),
            _ => tables.push(Table { name: table, columns: vec![column] }),
        }
    }
    tables
}

// `int8` gives `INT8`, the `_text` arrays `TEXT[]`, as sqlx names the column types
fn postgres_column_type(udt: &str) -> String {
    match udt.strip_prefix('_') {
        Some(element) => format!("{}[]", element.to_uppercase()),
        None => udt.to_uppercase(),
    }
}

// Tables of the database of `url`, read without writing anything,
// a relative SQLite file is looked up from `root`, the crate root, when missing from the working directory
async fn database_tables(url: &str, root: &Path) -> Result<Vec<Table>, String> {
    use sqlx::Connection;
    let error = |e: sqlx::Error| e.to_string();
    match url_dialect(url) {
        Some(Dialect::Sqlite) => {
            use std::str::FromStr;
            let mut options = sqlx::sqlite::SqliteConnectOptions::from_str(url).map_err(error)?;
            let file = options.get_filename().to_path_buf();
            if file.is_relative() && !file.exists() && root.join(&file).exists() {
                options = options.filename(root.join(&file));
            }
            let mut conn = sqlx::SqliteConnection::connect_with(&options.read_only(true).create_if_missing(false)).await.map_err(error)?;
            // the CREATE TABLE statements, as for the migrations
            let statements: Vec<Option<String>> = sqlx::query_scalar(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name <> $1 ORDER BY name",
            )
            .bind(SQLX_MIGRATIONS_TABLE)
            .fetch_all(&mut conn)
            .await
            .map_err(error)?;
            Ok(statements.iter().flatten().flat_map(|sql| parse_tables(sql)).collect())
        }
        Some(Dialect::Postgres) => {
            let mut conn = sqlx::PgConnection::connect(url).await.map_err(error)?;
            let rows: Vec<(String, String, String, bool)> = sqlx::query_as(POSTGRES_COLUMNS).fetch_all(&mut conn).await.map_err(error)?;
            Ok(group_columns(rows.into_iter().map(|(table, name, udt, unique)| {
                (table, Column { name, sql_type: postgres_column_type(&udt), unique })
            })))
        }
        Some(Dialect::Mysql) => {
            let mut conn = sqlx::MySqlConnection::connect(url).await.map_err(error)?;
            let rows: Vec<(String, String, String, i64)> = sqlx::query_as(MYSQL_COLUMNS).fetch_all(&mut conn).await.map_err(error)?;
            Ok(group_columns(rows.into_iter().map(|(table, name, ty, key)| {
                (table, Column { name, sql_type: ty.to_uppercase(), unique: key != 0 })
            })))
        }
        None => Err("unsupported scheme, use sqlite:, postgres: or mysql:".to_string()),
    }
}

// Tables proposed to the foreign keys: those of the DATABASE_URL database when it is set and reachable,
// those created by the migrations otherwise
fn known_tables(dialect: Dialect) -> Result<Vec<Table>, Error> {
    let cwd = std::env::current_dir()?;
    let migrations = migrations_dir(&cwd);
    let root = migrations.parent().map_or(PathBuf::new(), Path::to_path_buf);
    if let Ok(url) = std::env::var("DATABASE_URL") {
        if let Some(db) = url_dialect(&url).filter(|db| *db != dialect) {
            println!("{}", warning(&format!("`DATABASE_URL` is a {} database and the migration targets {}", db.name(), dialect.name())));
        }
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        match runtime.block_on(database_tables(&url, &root)) {
            Ok(tables) => {
                println!("{}", success(&format!("{} table{} read from `DATABASE_URL`", tables.len(), if tables.len() > 1 { "s" } else { "" })));
                return Ok(tables);
            }
            Err(e) => println!("{}", warning(&format!("Tables of `DATABASE_URL` not read ({}), the migrations are read instead", e))),
        }
    }
    let tables = migration_tables(&migrations);
    if tables.is_empty() {
        println!("{}", warning(&format!("No table in {}, the fields can only reference the model itself", migrations.display())));
    } else {
        println!("{}", success(&format!("{} table{} read from {}", tables.len(), if tables.len() > 1 { "s" } else { "" }, migrations.display())));
    }
    Ok(tables)
}

// Recap of the declared fields, with the `id` and the timestamps declared by the CLI dimmed
fn fields_summary(fields: &[Field], timestamps: bool) -> String {
    let declared: Vec<(&str, &str)> = std::iter::once(("id", "Id"))
        .chain(timestamps.then_some(TIMESTAMP_COLUMNS.map(|c| (c, TIMESTAMP_TYPE))).into_iter().flatten())
        .collect();
    let width = fields.iter().map(|f| f.name.len()).chain(declared.iter().map(|(n, _)| n.len())).max().unwrap_or(0);
    let mut lines = vec![format!("\n{}", bold(&format!("{} field{} declared", fields.len(), if fields.len() > 1 { "s" } else { "" })))];
    lines.push(format!("  {}", dim(&format!("{:<width$}: Id ({})", "id", ID_TYPE, width = width))));
    lines.extend(fields.iter().map(|f| format!("  {}", field_line(f, width))));
    lines.extend(declared.iter().skip(1).map(|(n, ty)| format!("  {}", dim(&format!("{:<width$}: {}", n, ty, width = width)))));
    lines.join("\n") + "\n"
}

// Bodies of find, list, delete, save and update, as left to the user without --sqlx
const EMPTY_BODIES: [&str; 5] = [
    "            // fetch from somwhere with id",
    "            // list",
    "            // hard or soft delete",
    "            // persist",
    "            // update in db",
];

// Pagination parameters of the list query, read by SqlxModel, only generated with --sqlx
const LIST_QUERY_FIELDS: &str = r#"
        /// Number of rows to skip
        pub offset: Option<usize>,
        /// Maximum number of rows to return (20 by default, 100 at most)
        pub limit: Option<usize>,
    "#;

// Clamps the pagination parameters of the list query before binding them
const LIST_PAGINATION: &str = "            let offset = query.offset.unwrap_or(0) as i64;
            let limit = query.limit.map_or(DEFAULT_LIMIT, |l| (l as i64).min(MAX_LIMIT));";

const RELATION_LIMITS: &str = r#"
    /// Number of children returned when no limit is given
    const DEFAULT_LIMIT: i64 = 20;
    /// Maximum number of children returned
    const MAX_LIMIT: i64 = 100;"#;

// Database targeted by the sqlx queries and the migration
#[derive(Debug, Clone, Copy, PartialEq)]
enum Dialect {
    Sqlite,
    Postgres,
    Mysql,
}

// Column types of the field types, as sqlx declares them (`sqlx::Type::type_info`)
// so the columns decode into the fields, the field types are listed with the path used in the models
macro_rules! sql_types {
    ($db:ty; $($ty:ty),* $(,)?) => {
        vec![$( (stringify!($ty), <$ty as sqlx::Type<$db>>::type_info().name().to_string()) ),*]
    };
}

fn sqlite_types() -> Vec<(&'static str, String)> {
    sql_types!(sqlx::Sqlite;
        String, i8, i16, i32, u8, u16, i64, u32, u64, f32, f64, bool,
        DateTime<Utc>, NaiveDateTime, NaiveDate, NaiveTime, Vec<u8>,
    )
}

// Replaces the column types sqlx declares by the ones of `overrides`
fn with_overrides(mut types: Vec<(&'static str, String)>, overrides: &[(&str, &str)]) -> Vec<(&'static str, String)> {
    for (ty, sql) in types.iter_mut() {
        if let Some((_, over)) = overrides.iter().find(|(t, _)| t == ty) {
            *sql = over.to_string();
        }
    }
    types
}

// Strings are bounded as for MySQL, where sqlx declares an unbounded TEXT
const POSTGRES_OVERRIDES: &[(&str, &str)] = &[("String", "VARCHAR(255)")];

// PostgreSQL has no unsigned integers and sqlx maps i8 to "char", Vec<T> are arrays
fn postgres_types() -> Vec<(&'static str, String)> {
    let types = sql_types!(sqlx::Postgres;
        String, i16, i32, i64, f32, f64, bool,
        DateTime<Utc>, NaiveDateTime, NaiveDate, NaiveTime, Vec<u8>,
        Vec<String>, Vec<i16>, Vec<i32>, Vec<i64>, Vec<f32>, Vec<f64>, Vec<bool>,
        Vec<DateTime<Utc>>, Vec<NaiveDateTime>, Vec<NaiveDate>, Vec<NaiveTime>, Vec<Vec<u8>>,
    );
    with_overrides(types, POSTGRES_OVERRIDES)
}

// sqlx names a VARCHAR without its length, which is not a valid column type.
// sqlx writes DateTime<Utc> as its UTC date and time, stored as is by DATETIME
// where TIMESTAMP (the sqlx type) would convert it from the session time zone,
// and with microseconds as the other databases
const MYSQL_OVERRIDES: &[(&str, &str)] = &[
    ("String", "VARCHAR(255)"),
    ("DateTime<Utc>", "DATETIME(6)"),
    ("NaiveDateTime", "DATETIME(6)"),
    ("NaiveTime", "TIME(6)"),
];

fn mysql_types() -> Vec<(&'static str, String)> {
    let types = sql_types!(sqlx::MySql;
        String, i8, i16, i32, i64, u8, u16, u32, u64, f32, f64, bool,
        DateTime<Utc>, NaiveDateTime, NaiveDate, NaiveTime, Vec<u8>,
    );
    with_overrides(types, MYSQL_OVERRIDES)
}

impl Dialect {
    fn from_flags(postgres: bool, mysql: bool) -> Dialect {
        match (postgres, mysql) {
            (true, _) => Dialect::Postgres,
            (_, true) => Dialect::Mysql,
            _ => Dialect::Sqlite,
        }
    }

    // The flag selecting the database on the command line
    fn flag(self) -> &'static str {
        match self {
            Dialect::Sqlite => "--sqlite",
            Dialect::Postgres => "--postgres",
            Dialect::Mysql => "--mysql",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Dialect::Sqlite => "SQLite",
            Dialect::Postgres => "PostgreSQL",
            Dialect::Mysql => "MySQL",
        }
    }

    fn sql_types(self) -> &'static [(&'static str, String)] {
        static SQLITE: OnceLock<Vec<(&str, String)>> = OnceLock::new();
        static POSTGRES: OnceLock<Vec<(&str, String)>> = OnceLock::new();
        static MYSQL: OnceLock<Vec<(&str, String)>> = OnceLock::new();
        match self {
            Dialect::Sqlite => SQLITE.get_or_init(sqlite_types),
            Dialect::Postgres => POSTGRES.get_or_init(postgres_types),
            Dialect::Mysql => MYSQL.get_or_init(mysql_types),
        }
    }

    // Column of the `id: i64` primary key
    fn id_column(self) -> &'static str {
        match self {
            Dialect::Sqlite => "id INTEGER PRIMARY KEY AUTOINCREMENT",
            Dialect::Postgres => "id BIGSERIAL PRIMARY KEY",
            Dialect::Mysql => "id BIGINT AUTO_INCREMENT PRIMARY KEY",
        }
    }

    fn sql_type(self, ty: &str) -> Option<&'static str> {
        // chrono types can be written with their path
        let ty = ty.trim_start_matches("chrono::");
        self.sql_types().iter().find(|(t, _)| *t == ty).map(|(_, sql)| sql.as_str())
    }

    // Column type of a field type and whether it is NOT NULL, `Option<T>` fields are nullable,
    // None when the type has no column type
    fn sql_column_type(self, ty: &str) -> Option<(&'static str, bool)> {
        let ty: String = ty.chars().filter(|c| !c.is_whitespace()).collect();
        match ty.strip_prefix("Option<").and_then(|t| t.strip_suffix('>')) {
            Some(inner) => self.sql_type(inner).map(|sql| (sql, false)),
            None => self.sql_type(&ty).map(|sql| (sql, true)),
        }
    }

    // The column type of a field type, `length` replacing the one of a VARCHAR
    fn column_type(self, ty: &str, length: Option<u32>) -> Option<String> {
        self.sql_column_type(ty).map(|(sql, _)| self.with_length(sql, length))
    }

    fn with_length(self, sql: &str, length: Option<u32>) -> String {
        match (varchar_length(sql), length) {
            (Some(_), Some(length)) => format!("VARCHAR({})", length),
            _ => sql.to_string(),
        }
    }

    // PostgreSQL limits a VARCHAR to 10485760 characters, MySQL to 65535 bytes in a row,
    // 16383 characters of utf8mb4
    fn max_varchar_length(self) -> u32 {
        match self {
            Dialect::Mysql => 16383,
            _ => 10_485_760,
        }
    }

    // `$1, $2...` for SQLite and PostgreSQL, `?` for MySQL
    // Whether a column of type `a` can reference a column of type `b`, after the aliases of the database,
    // SQLite does not check the types of the foreign keys
    fn same_column_type(self, a: &str, b: &str) -> bool {
        let canonical = |sql: &str| {
            let sql = sql.split_whitespace().collect::<Vec<_>>().join(" ").to_ascii_uppercase();
            let alias = match (self, sql.as_str()) {
                (Dialect::Postgres, "BIGSERIAL" | "SERIAL8" | "BIGINT") => "INT8",
                (Dialect::Postgres, "SERIAL" | "SERIAL4" | "INTEGER" | "INT") => "INT4",
                (Dialect::Postgres, "SMALLSERIAL" | "SERIAL2" | "SMALLINT") => "INT2",
                (Dialect::Postgres, "DOUBLE PRECISION") => "FLOAT8",
                (Dialect::Postgres, "REAL") => "FLOAT4",
                (Dialect::Postgres, "BOOLEAN") => "BOOL",
                (Dialect::Postgres, "TIMESTAMP WITH TIME ZONE") => "TIMESTAMPTZ",
                // a VARCHAR, of any length, can reference a TEXT
                (Dialect::Postgres, s) if s == "TEXT" || s.starts_with("VARCHAR") || s.starts_with("CHARACTER VARYING") => "TEXT",
                (Dialect::Mysql, "INTEGER") => "INT",
                (Dialect::Mysql, "BOOL" | "TINYINT(1)") => "BOOLEAN",
                _ => return sql,
            };
            alias.to_string()
        };
        self == Dialect::Sqlite || canonical(a) == canonical(b)
    }

    fn placeholders(self, count: usize) -> Vec<String> {
        match self {
            Dialect::Mysql => vec!["?".to_string(); count],
            _ => (1..=count).map(|i| format!("${}", i)).collect(),
        }
    }
}

// A query_as statement of `result` rows bound to `var`
fn sqlx_fetch_all(var: &str, result: &str, sql: &str, binds: &[String]) -> String {
    let binds: String = binds
        .iter()
        .map(|b| format!("\n            .bind({})", b))
        .collect();
    format!(
        "            let {} = sqlx::query_as::<_, {}>(\n                \"{}\",\n            ){}\n            .fetch_all(&state.pool)\n            .await?;",
        var, result, sql, binds
    )
}

// Fails with the fields whose type has no column type in the `dialect` database
fn render_migration(name: &str, fields: &[Field], timestamps: bool, dialect: Dialect) -> Result<String, String> {
    let table = name.to_lowercase();
    let mut columns = vec![dialect.id_column().to_string()];
    columns.extend(column_definitions(fields, dialect)?);
    if timestamps {
        // nullable, as their Option<DateTime<Utc>> fields
        let sql = dialect.sql_type("DateTime<Utc>").unwrap_or_default();
        columns.extend(TIMESTAMP_COLUMNS.map(|c| format!("{} {}", c, sql)));
    }
    // table constraints, the inline REFERENCES are ignored by MySQL,
    // the foreign keys are named so that MySQL can drop them
    columns.extend(fields.iter().filter(|f| f.unique).map(|f| format!("UNIQUE ({})", f.name)));
    columns.extend(fields.iter().filter_map(|f| {
        f.references.as_ref().map(|r| format!("CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({})", foreign_key_name(&table, &f.name), f.name, r.table, r.column))
    }));
    Ok(format!(
        "CREATE TABLE IF NOT EXISTS {} (\n    {}\n);\n",
        table,
        columns.join(",\n    ")
    ))
}

// `name TYPE [NOT NULL]`, the column of a field, None when its type has no column type in the `dialect` database
fn column_definition(field: &Field, dialect: Dialect) -> Option<String> {
    dialect.sql_column_type(&field.ty).map(|(sql, not_null)| {
        let sql = dialect.with_length(sql, field.length);
        if not_null { format!("{} {} NOT NULL", field.name, sql) } else { format!("{} {}", field.name, sql) }
    })
}

// The columns of the fields, fails with the fields whose type has no column type in the `dialect` database
fn column_definitions(fields: &[Field], dialect: Dialect) -> Result<Vec<String>, String> {
    let unmapped: Vec<String> = fields
        .iter()
        .filter(|f| column_definition(f, dialect).is_none())
        .map(|f| format!("{}: {}", f.name, f.ty))
        .collect();
    if !unmapped.is_empty() {
        return Err(format!("no {} column type for {}", dialect.name(), unmapped.join(", ")));
    }
    Ok(fields.iter().filter_map(|f| column_definition(f, dialect)).collect())
}

fn foreign_key_name(table: &str, column: &str) -> String {
    format!("fk_{}_{}", table, column)
}

// Name of the unique index of a column, the one PostgreSQL gives to a UNIQUE constraint
fn unique_index_name(table: &str, column: &str) -> String {
    format!("{}_{}_key", table, column)
}

// Migration adding the columns of `fields` to `table`, the columns `defaults` sets on the existing rows are given
// by the field name, the unique columns get a unique index and the foreign keys a named constraint,
// declared inline by SQLite, which cannot add a constraint to a table
fn render_add_columns(table: &str, fields: &[Field], defaults: &[(String, String)], dialect: Dialect) -> Result<String, String> {
    let columns = column_definitions(fields, dialect)?;
    let mut sql = String::new();
    for (field, column) in fields.iter().zip(columns) {
        sql += &format!("ALTER TABLE {} ADD COLUMN {}", table, column);
        if let Some((_, default)) = defaults.iter().find(|(name, _)| *name == field.name) {
            sql += &format!(" DEFAULT {}", default);
        }
        if let (Some(r), Dialect::Sqlite) = (&field.references, dialect) {
            sql += &format!(" REFERENCES {} ({})", r.table, r.column);
        }
        sql += ";\n";
    }
    // SQLite refuses ADD COLUMN ... UNIQUE
    for field in fields.iter().filter(|f| f.unique) {
        sql += &format!("CREATE UNIQUE INDEX {} ON {} ({});\n", unique_index_name(table, &field.name), table, field.name);
    }
    if dialect != Dialect::Sqlite {
        for field in fields {
            if let Some(r) = &field.references {
                sql += &format!(
                    "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({});\n",
                    table, foreign_key_name(table, &field.name), field.name, r.table, r.column
                );
            }
        }
    }
    Ok(sql)
}

fn is_nullable(ty: &str) -> bool {
    ty.trim_start().starts_with("Option<")
}

// Why the database refuses to add the column of `field` with the `default`, SQLite adds a NOT NULL column
// only with a default, a foreign key only with a NULL default, and no default that is not a constant
fn add_column_error(field: &Field, default: Option<&str>, dialect: Dialect) -> Option<String> {
    let default = default.map(str::trim).filter(|d| !d.eq_ignore_ascii_case("NULL"));
    if dialect != Dialect::Sqlite {
        return None;
    }
    if let Some(d) = default {
        let upper = d.to_ascii_uppercase();
        if d.starts_with('(') || ["CURRENT_TIME", "CURRENT_DATE", "CURRENT_TIMESTAMP"].iter().any(|f| upper.contains(f)) {
            return Some(format!("SQLite adds a column only with a constant default, not `{}`", d));
        }
        if field.references.is_some() {
            return Some(format!("SQLite adds the foreign key `{}` only with a NULL default, make it optional", field.name));
        }
    }
    if default.is_none() && !is_nullable(&field.ty) {
        return Some(format!("SQLite adds the NOT NULL column `{}` only with a default", field.name));
    }
    None
}

// Default proposed for the existing rows of a non-optional column of type `ty`, None when there is none to propose
fn suggested_default(ty: &str, dialect: Dialect) -> Option<&'static str> {
    let ty: String = ty.chars().filter(|c| !c.is_whitespace()).collect();
    let ty = ty.trim_start_matches("chrono::");
    let default = match (ty, dialect) {
        ("String", _) => "''",
        ("i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64", _) => "0",
        ("bool", _) => "FALSE",
        ("DateTime<Utc>" | "NaiveDateTime", Dialect::Sqlite) => "'1970-01-01 00:00:00'",
        ("DateTime<Utc>" | "NaiveDateTime", Dialect::Postgres) => "CURRENT_TIMESTAMP",
        // the fractional seconds of DATETIME(6)
        ("DateTime<Utc>" | "NaiveDateTime", Dialect::Mysql) => "CURRENT_TIMESTAMP(6)",
        ("NaiveDate", Dialect::Sqlite) => "'1970-01-01'",
        ("NaiveDate", Dialect::Postgres) => "CURRENT_DATE",
        // an expression default, MySQL 8.0.13 and later
        ("NaiveDate", Dialect::Mysql) => "(CURRENT_DATE)",
        ("NaiveTime", _) => "'00:00:00'",
        ("Vec<u8>", Dialect::Sqlite) => "X''",
        ("Vec<u8>", Dialect::Postgres) => "''",
        // no literal default for a BLOB
        ("Vec<u8>", Dialect::Mysql) => "('')",
        (ty, Dialect::Postgres) if ty.starts_with("Vec<") => "'{}'",
        _ => return None,
    };
    Some(default)
}

// The default of the column of `field` for the existing rows, `flag` (--default) when set, None for an optional field,
// asked otherwise, `?` making the field optional instead, which a SQLite foreign key must be
fn read_default<R: BufRead, W: Write>(input: &mut R, output: &mut W, field: &mut Field, flag: Option<&str>, dialect: Dialect) -> Result<Option<String>, Error> {
    if let Some(default) = flag {
        return Ok(Some(default.to_string()));
    }
    if is_nullable(&field.ty) {
        return Ok(None);
    }
    if dialect == Dialect::Sqlite && field.references.is_some() {
        field.ty = format!("Option<{}>", field.ty);
        writeln!(output, "{}", warning(&format!("SQLite adds a foreign key column only when it is nullable, `{}` is now {}", field.name, field.ty)))?;
        return Ok(None);
    }
    let suggested = suggested_default(&field.ty, dialect);
    loop {
        let message = format!(
            "{} {} {}{} ",
            cyan("?"),
            bold(&format!("Default of {} for the existing rows ›", cyan(&format!("`{}`", field.name)))),
            dim("(SQL value, `?` makes the field optional)"),
            suggested.map_or(String::new(), |d| dim(&format!(" [{}]", d)))
        );
        let answer = prompt(input, output, &message)?.unwrap_or_default();
        if answer == "?" {
            field.ty = format!("Option<{}>", field.ty);
            writeln!(output, "  {} {}", green("✔"), field_line(field, 0))?;
            return Ok(None);
        }
        let default = match (answer.is_empty(), suggested) {
            (false, _) => answer,
            (true, Some(suggested)) => suggested.to_string(),
            (true, None) => {
                writeln!(output, "{}", failure(&format!("`{}` has no default to propose, enter one or `?`", field.ty)))?;
                continue;
            }
        };
        match add_column_error(field, Some(&default), dialect) {
            Some(error) => writeln!(output, "{}", failure(&error))?,
            None => return Ok(Some(default)),
        }
    }
}

// Octopus drawn at the top of every generated file
const LOGO: &str = r#"
                        -----
                    -------------
                  -----  ----------
                 ---  --------------
                ---  ----------------
                --- -----------------
                --- -----------------
                --- -----------------
                ---------------------
      -----      -------------------       -----
     -------      --  ---------- --      -------
         ----      ---------------      -----
          ---      ---------------      ----
         ----     -----------------     ----
       ------   ----------------------   ------
   --------  ---------------------------  --------
  ------   -------------------------- ----   -------
 ----    -----  ---------- --- ------- -----    -----
----  ------  -------- --- --- ---- ---  ------  ----
----        ---- ----  --- ---- ---- -----       ----
 ----   ------  ----  ---- ----  ----   ------  -----
 ------      ------   ---- -----  ------      ------
   ---------------    ----  ----    --------------
     ----------       ----  ----      ----------
                      ----  ----
                ---   ---- -----   --
              ------  ---- ----- -------
             -------  ---- ----- --------
             ----    ----   -----    ----
             -----------     -----------
              ---------        --------
"#;

// LOGO, commented with `comment` (`//` for Rust, `--` for SQL)
fn generated_header(comment: &str) -> String {
    LOGO.trim_matches('\n')
        .lines()
        .map(|line| format!("{} {}", comment, line).trim_end().to_string() + "\n")
        .collect::<String>()
        + "\n"
}

// `content` of a generated file, preceded by its header
fn with_header(comment: &str, content: &str) -> String {
    generated_header(comment) + content.trim_start_matches('\n')
}

// Exits when `path` exists and is not to be overwritten, before prompting for anything
fn refuse_overwrite(path: &str, force: bool, what: &str) {
    if !force && Path::new(path).exists() {
        eprintln!("{}", failure(&format!("{} already exists, use --force to overwrite it, {} not generated", path, what)));
        process::exit(1);
    }
}

// `file` in the `output` folder when given, else in the src folder of the working directory when it exists,
// in the working directory otherwise
fn source_path(cwd: &Path, output: Option<&Path>, file: &str) -> String {
    match output {
        Some(dir) => dir.join(file).display().to_string(),
        None if cwd.join("src").is_dir() => format!("src/{}", file),
        None => file.to_string(),
    }
}

// Writes a generated source file at `path`, creating its folder (the --output one) when missing
fn write_source(path: &str, content: &str) -> Result<(), Error> {
    if let Some(dir) = Path::new(path).parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, with_header("//", content))
}

// `<timestamp>_<suffix>.sql` in the migrations folder next to src
fn migration_path(suffix: &str) -> Result<PathBuf, Error> {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    Ok(migrations_dir(&std::env::current_dir()?).join(format!("{}_{}.sql", migration_timestamp(secs), suffix)))
}

// Writes the migration at `path` (see `migration_path`), creating the migrations folder
fn write_migration(path: &Path, sql: &str) -> Result<(), Error> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, with_header("--", sql))?;
    println!("{}", success(&format!("Successfully generated migration {}", path.display())));
    Ok(())
}

// Recap of the files about to be written, each with whether it overwrites an existing file
fn changes_summary(files: &[(String, bool)]) -> String {
    let mut lines = vec![bold(&format!("{} file{} to write", files.len(), if files.len() > 1 { "s" } else { "" }))];
    lines.extend(files.iter().map(|(path, overwrites)| {
        if *overwrites {
            format!("  {} {} {}", yellow("~"), path, yellow("(overwritten)"))
        } else {
            format!("  {} {}", green("+"), path)
        }
    }));
    lines.join("\n")
}

// Shows the recap of the files and asks for saving them, only an explicit no refuses,
// so that a session is not lost on an empty answer, and the fields can still be piped
fn confirm_save<R: BufRead, W: Write>(input: &mut R, output: &mut W, files: &[(String, bool)]) -> Result<bool, Error> {
    writeln!(output, "{}", changes_summary(files))?;
    let answer = prompt(input, output, &format!("{} {} {} ", cyan("?"), bold("Save the changes?"), dim("(Y/n)")))?;
    Ok(!matches!(answer.as_deref().map(str::to_lowercase).as_deref(), Some("n" | "no")))
}

// UTC timestamp prefixing the sqlx migrations, as YYYYMMDDHHMMSS
fn migration_timestamp(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    // civil date from days since 1970-01-01 (Howard Hinnant's algorithm)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    format!(
        "{:04}{:02}{:02}{:02}{:02}{:02}",
        year, month, day, rem / 3600, rem % 3600 / 60, rem % 60
    )
}

// migrations folder of the crate, next to its src folder, relative to the working directory
fn migrations_dir(cwd: &Path) -> PathBuf {
    let mut dir = PathBuf::new();
    for ancestor in cwd.ancestors() {
        dir.push("..");
        if ancestor.file_name().map_or(false, |n| n == "src") {
            return dir.join("migrations");
        }
    }
    // outside of src, the working directory is taken as the crate root
    PathBuf::from("migrations")
}

// chrono types used by the timestamps and the date fields
fn chrono_names(fields: &[Field], timestamps: bool) -> Vec<&'static str> {
    // whole type names, `NaiveDateTime` must not import `DateTime`
    let uses = |name: &str| {
        fields
            .iter()
            .any(|f| f.ty.split(|c: char| !c.is_ascii_alphanumeric() && c != '_').any(|t| t == name))
    };
    ["DateTime", "NaiveDate", "NaiveDateTime", "NaiveTime", "Utc"]
        .into_iter()
        .filter(|name| uses(name) || (timestamps && (*name == "DateTime" || *name == "Utc")))
        .collect()
}

// chrono import needed by the timestamps and the date fields, empty when none is used
fn chrono_imports(fields: &[Field], timestamps: bool) -> String {
    let names = chrono_names(fields, timestamps);
    if names.is_empty() {
        String::new()
    } else {
        format!("\n    use chrono::{{{}}};", names.join(", "))
    }
}

// Keywords a field can be named after as a raw identifier (`r#type`), its column keeps the plain name
const RAW_KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "do", "dyn", "else", "enum", "extern", "false",
    "final", "fn", "for", "gen", "if", "impl", "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub",
    "ref", "return", "static", "struct", "trait", "true", "try", "type", "typeof", "unsafe", "unsized", "use", "virtual", "where",
    "while", "yield",
];

// The identifier of a field: `type` gives `r#type`
fn field_ident(name: &str) -> String {
    if RAW_KEYWORDS.contains(&name) { format!("r#{}", name) } else { name.to_string() }
}

fn struct_fields(fields: &[Field]) -> String {
    fields
        .iter()
        .map(|f| format!("\n        pub {}: {},", field_ident(&f.name), f.ty))
        .collect()
}

// `#[sqlx_model(...)]` of the model structs, `model` names the model returned by `save`,
// `table` the table queried when it is not the lowercase model name
fn sqlx_model_attribute(dialect: Dialect, model: Option<&str>, table: Option<&str>, timestamps: bool, soft_delete: bool) -> String {
    let database = match dialect {
        Dialect::Sqlite => "sqlite",
        Dialect::Postgres => "postgres",
        Dialect::Mysql => "mysql",
    };
    let mut args = vec![format!("database = \"{}\"", database)];
    if let Some(model) = model {
        args.push(format!("model = \"{}\"", model));
    }
    if let Some(table) = table {
        args.push(format!("table = \"{}\"", table));
    }
    if timestamps {
        args.push("timestamps".to_string());
    }
    if soft_delete {
        args.push("soft_delete".to_string());
    }
    format!("\n    #[sqlx_model({})]", args.join(", "))
}

// The model of `name`, on its lowercase table
#[cfg(test)]
fn render_model(name: &str, openapi: bool, graphql: bool, sqlx: bool, timestamps: bool, fields: &[Field], dialect: Dialect) -> String {
    render_table_model(name, None, openapi, graphql, sqlx, timestamps, fields, dialect)
}

// The model of `name`, its sqlx queries on `table` when set, on the lowercase name otherwise
#[allow(clippy::too_many_arguments)]
fn render_table_model(name: &str, table: Option<&str>, openapi: bool, graphql: bool, sqlx: bool, timestamps: bool, fields: &[Field], dialect: Dialect) -> String {
    // the module of the model is named after its table
    let module = table.map_or_else(|| to_snake_case(name), String::from);
    // the derives default to the lowercase model name
    let table = table.filter(|t| *t != name.to_lowercase());
    let (imports, derives, configure) = if openapi {
        (OPENAPI_IMPORTS, OPENAPI_DERIVES, OPENAPI_CONFIGURE)
    } else {
        (ENDPOINT_IMPORTS, "", ENDPOINT_CONFIGURE)
    };
    let field_lines = struct_fields(fields);
    // keeps the blank line of the empty creatable struct
    let new_fields = if fields.is_empty() { "\n" } else { &field_lines };
    let chrono_imports = chrono_imports(fields, timestamps);
    let (model_fields, updatable_fields) = if timestamps {
        let timestamp = |name: &str| Field { name: name.to_string(), ty: TIMESTAMP_TYPE.to_string(), references: None, unique: false, length: None };
        (
            field_lines.clone() + &struct_fields(&TIMESTAMP_COLUMNS.map(timestamp)),
            field_lines.clone() + &struct_fields(&[timestamp("updated_at")]),
        )
    } else {
        (field_lines.clone(), field_lines.clone())
    };
    // with --sqlx, the derives implement the model traits, left to fill otherwise
    let tpl = if sqlx {
        let trait_imports = if graphql { format!("{}{}", SQLX_TRAIT_IMPORTS, GRAPHQL_SQLX_TRAIT_IMPORTS) } else { SQLX_TRAIT_IMPORTS.to_string() };
        MODEL_TPL
            .replace("{trait_imports}", &trait_imports)
            .replace("{model_impl}", "")
            .replace("{new_impl}", "")
            .replace("{updatable_impl}", "")
            .replace("{result_types}", "")
            .replace("{list_query_fields}", LIST_QUERY_FIELDS)
            .replace("{model_derives}", ", sqlx::FromRow, HttpFindListDelete, SqlxModel")
            .replace("{new_derives}", ", HttpCreate, SqlxNewModel")
            .replace("{updatable_derives}", ", sqlx::FromRow, HttpUpdate, SqlxUpdatableModel")
            .replace("{model_sqlx}", &sqlx_model_attribute(dialect, None, table, timestamps, timestamps))
            .replace("{new_sqlx}", &sqlx_model_attribute(dialect, Some(name), table, timestamps, false))
            .replace("{updatable_sqlx}", &sqlx_model_attribute(dialect, None, table, timestamps, timestamps))
    } else {
        let [find_body, list_body, delete_body, save_body, update_body] = EMPTY_BODIES;
        MODEL_TPL
            .replace("{trait_imports}", TRAIT_IMPORTS)
            .replace("{model_impl}", &MODEL_IMPL.replace("{find_body}", find_body).replace("{list_body}", list_body).replace("{delete_body}", delete_body))
            .replace("{new_impl}", &NEW_IMPL.replace("{save_body}", save_body))
            .replace("{updatable_impl}", &UPDATABLE_IMPL.replace("{update_body}", update_body))
            .replace("{result_types}", "\n    pub type ListResult = Vec<{entity}>;\n    pub type DeleteResult = {entity};")
            .replace("{list_query_fields}", "")
            .replace("{model_derives}", ", HttpFindListDelete")
            .replace("{new_derives}", ", HttpCreate")
            .replace("{updatable_derives}", ", HttpUpdate")
            .replace("{model_sqlx}", "")
            .replace("{new_sqlx}", "")
            .replace("{updatable_sqlx}", "")
    };
    let (graphql_imports, graphql_output_derives, graphql_input_derives, graphql_resolvers) = if graphql {
        // the list query of --sqlx paginates, its fields are the arguments of the list resolver
        let (list_args, list_query) = if sqlx { (", offset: Option<usize>, limit: Option<usize>", " offset, limit ") } else { ("", "") };
        // the GraphQL fields are named after the model, `book_page` and `book_pages`
        let field = to_snake_case(name);
        let resolvers = GRAPHQL_RESOLVERS
            .replace("{list_args}", list_args)
            .replace("{list_query}", list_query)
            .replace("{plural}", &pluralize(&field))
            .replace("{field}", &field)
            .replace("{module}", &module)
            .replace("{relations_marker}", GRAPHQL_RELATIONS_MARKER);
        (GRAPHQL_IMPORTS, ", SimpleObject", ", InputObject", resolvers)
    } else {
        ("", "", "", String::new())
    };
    tpl.replace("{graphql_resolvers}", &graphql_resolvers)
        .replace("{graphql_complex}", if graphql { "\n    #[graphql(complex)]" } else { "" })
        .replace("{graphql_imports}", graphql_imports)
        .replace("{graphql_output_derives}", graphql_output_derives)
        .replace("{graphql_input_derives}", graphql_input_derives)
        .replace("{model_fields}", &model_fields)
        .replace("{updatable_fields}", &updatable_fields)
        .replace("{chrono_imports}", &chrono_imports)
        .replace("{new_fields}", new_fields)
        .replace("{openapi_imports}", imports)
        .replace("{openapi_derives}", derives)
        .replace("{openapi_configure}", configure)
        .replace("{id_type}", ID_TYPE)
        .replace("{entity}", name)
        .replace("{entity_lower_case}", &name.to_lowercase())
}

const TRAIT_IMPORTS: &str = "
        HttpCreate,
        HttpFindListDelete,
        HttpUpdate,
        Model,
        NewModel,
        UpdatableModel,
        octopux_info,
        anyhow::Result,
        async_trait,";

const SQLX_TRAIT_IMPORTS: &str = "
        HttpCreate,
        HttpFindListDelete,
        HttpUpdate,
        SqlxModel,
        SqlxNewModel,
        SqlxUpdatableModel,
        octopux_info,";

const MODEL_IMPL: &str = r#"

    #[async_trait]
    impl Model<Id, FindQuery, ListQuery, ListResult, DeleteQuery, DeleteResult, AppState> for {entity} {
        async fn find(id: Id, _query: &FindQuery, _state: &AppState) -> Result<Box<{entity}>> {
{find_body}
        }
        async fn list(_query: &ListQuery, _state: &AppState) -> Result<ListResult> {
{list_body}
        }
        async fn delete(mut self: Self, _query: &DeleteQuery, _state: &AppState) -> Result<DeleteResult> {
{delete_body}
        }
    }"#;

const NEW_IMPL: &str = r#"
    #[async_trait]
    impl NewModel<{entity}, SaveQuery, AppState> for New{entity} {
        async fn save(self: Self, _query: &SaveQuery, _state: &AppState) -> Result<{entity}> {
{save_body}
        }
    }"#;

const UPDATABLE_IMPL: &str = r#"
    #[async_trait]
    impl UpdatableModel<Updatable{entity}, UpdateQuery, AppState> for Updatable{entity} {
        async fn update(mut self: Self, _query: &UpdateQuery, _state: &AppState) -> Result<Updatable{entity}> {
{update_body}
        }
    }"#;

const MODEL_TPL: &str = r#"
    // The application state, declared (or re-exported) at the root of the crate
    use crate::AppState;
    use serde::{Serialize, Deserialize};
    use octopux::{{trait_imports}
    };{chrono_imports}{openapi_imports}{graphql_imports}

    #[derive(Default, Deserialize{openapi_derives})]
    pub struct FindQuery {}
    #[derive(Deserialize{openapi_derives})]
    pub struct ListQuery {{list_query_fields}}
    #[derive(Deserialize{openapi_derives})]
    pub struct DeleteQuery {}{result_types}
    #[derive(Deserialize{openapi_derives})]
    pub struct SaveQuery {}
    #[derive(Deserialize{openapi_derives})]
    pub struct UpdateQuery {}
    pub type Id = {id_type};

    #[derive(Default, Serialize, Deserialize{openapi_derives}{graphql_output_derives}{model_derives})]
    #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]{model_sqlx}
    #[octopux_info(path = "{entity_lower_case}")]{graphql_complex}
    pub struct {entity} {
        pub id: Id,{model_fields}
    }{model_impl}

    #[derive(Serialize, Deserialize{openapi_derives}{graphql_input_derives}{new_derives})]
    #[http_create(SaveQuery, AppState)]{new_sqlx}
    pub struct New{entity} {{new_fields}
    }{new_impl}

    #[derive(Serialize, Deserialize{openapi_derives}{graphql_input_derives}{updatable_derives})]
    #[http_update(Id, UpdateQuery, {entity}, FindQuery, AppState)]{updatable_sqlx}
    pub struct Updatable{entity} {
        pub id: Id,{updatable_fields}
    }{updatable_impl}{openapi_configure}{graphql_resolvers}
    "#;

// Plural of the last word of a snake_case name: `book` gives `books`, `category` gives `categories`
fn pluralize(name: &str) -> String {
    let consonant_y = name.ends_with('y') && !name[..name.len() - 1].ends_with(['a', 'e', 'i', 'o', 'u']);
    if consonant_y {
        format!("{}ies", &name[..name.len() - 1])
    } else if ["s", "x", "z", "ch", "sh"].iter().any(|end| name.ends_with(end)) {
        format!("{}es", name)
    } else {
        format!("{}s", name)
    }
}

// `favorite_books` gives `FavoriteBooks`
fn to_camel_case(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or(String::new(), |c| c.to_uppercase().chain(chars).collect())
        })
        .collect()
}

#[derive(Debug, PartialEq)]
struct Through {
    model: String,
    // join table, the snake_case model by default
    table: String,
    // column of the join table referencing the child
    child_key: String,
}

// A has-many relation of `parent`, its children referencing it by `foreign_key`,
// directly or through a join model
#[derive(Debug, PartialEq)]
struct Relation {
    parent: String,
    child: String,
    // tables of the parent and the child, their snake_case model by default
    parent_table: String,
    child_table: String,
    // last segment of the route
    name: String,
    foreign_key: String,
    through: Option<Through>,
}

impl Relation {
    fn new(parent: String, child: String, name: Option<String>, foreign_key: Option<String>, through: Option<String>, child_key: Option<String>) -> Relation {
        let child_snake = to_snake_case(&child);
        Relation {
            name: name.unwrap_or_else(|| pluralize(&child_snake)),
            foreign_key: foreign_key.unwrap_or_else(|| format!("{}_id", to_snake_case(&parent))),
            parent_table: to_snake_case(&parent),
            child_table: child_snake.clone(),
            through: through.map(|model| Through {
                table: to_snake_case(&model),
                model,
                child_key: child_key.unwrap_or_else(|| format!("{}_id", child_snake)),
            }),
            parent,
            child,
        }
    }

    // Replaces the default tables by the ones given
    fn with_tables(mut self, parent_table: Option<String>, child_table: Option<String>, through_table: Option<String>) -> Relation {
        if let Some(table) = parent_table {
            self.parent_table = table;
        }
        if let Some(table) = child_table {
            self.child_table = table;
        }
        if let (Some(through), Some(table)) = (self.through.as_mut(), through_table) {
            through.table = table;
        }
        self
    }

    // Module of the relation, next to the models, named after the table of the parent: `project_books`
    fn module(&self) -> String {
        format!("{}_{}", self.parent_table, self.name)
    }

    // Type the `HasMany` trait is implemented on: `ProjectBooks`
    fn type_name(&self) -> String {
        format!("{}{}", self.parent, to_camel_case(&self.name))
    }

    // Table and column holding the foreign key, to index
    fn foreign_key_column(&self) -> (String, &str) {
        let table = self.through.as_ref().map_or(&self.child_table, |through| &through.table);
        (table.clone(), &self.foreign_key)
    }
}

// Body of list_related: a page of the children, and the lookup of the parent when the page is empty,
// to answer 404 for an unknown parent, `timestamps` skips the soft deleted rows
fn relation_sqlx_body(relation: &Relation, timestamps: bool, dialect: Dialect) -> String {
    let parent = &relation.parent_table;
    let child = &relation.child_table;
    let placeholders = dialect.placeholders(3);
    let select = match &relation.through {
        None => format!(
            "SELECT * FROM {} WHERE {} = {}{} ORDER BY id LIMIT {} OFFSET {}",
            child,
            relation.foreign_key,
            placeholders[0],
            if timestamps { " AND deleted_at IS NULL" } else { "" },
            placeholders[1],
            placeholders[2]
        ),
        Some(through) => {
            let join = &through.table;
            format!(
                "SELECT {child}.* FROM {child} JOIN {join} ON {join}.{child_key} = {child}.id WHERE {join}.{fk} = {p1}{live} ORDER BY {child}.id LIMIT {p2} OFFSET {p3}",
                child = child,
                join = join,
                child_key = through.child_key,
                fk = relation.foreign_key,
                p1 = placeholders[0],
                live = if timestamps { format!(" AND {}.deleted_at IS NULL AND {}.deleted_at IS NULL", child, join) } else { String::new() },
                p2 = placeholders[1],
                p3 = placeholders[2],
            )
        }
    };
    let lookup = format!(
        "SELECT id FROM {} WHERE id = {}{}",
        parent,
        placeholders[0],
        if timestamps { " AND deleted_at IS NULL" } else { "" }
    );
    format!(
        "{}\n{}
            // the parent is only looked up when the page is empty
            if models.is_empty() {{
                let parent = sqlx::query_scalar::<_, Id>(
                    \"{}\",
                )
                .bind(id)
                .fetch_optional(&state.pool)
                .await?;
                if parent.is_none() {{
                    return Ok(None);
                }}
            }}
            Ok(Some(models))",
        LIST_PAGINATION,
        sqlx_fetch_all(
            "models",
            &relation.child,
            &select,
            &["id".to_string(), "limit".to_string(), "offset".to_string()],
        ),
        lookup
    )
}

// Resolver of the GraphQL field of the relation, called by the `#[ComplexObject]` of the parent
const RELATION_GRAPHQL_RESOLVER: &str = r#"

    // Resolves the `{relation_name}` field of the GraphQL {parent} type, declared in the `#[ComplexObject]` of its model
    pub async fn resolve(ctx: &async_graphql::Context<'_>, id: Id, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<{child}>> {
        let state = ctx.data::<actix_web::web::Data<AppState>>()?;
        let children = {relation}::list_related(id, &{relation}Query { offset, limit }, state.get_ref()).await?;
        // the parent was resolved, so it exists
        Ok(children.unwrap_or_default())
    }"#;

// Field of the relation in the `#[ComplexObject]` of the parent model
const RELATION_GRAPHQL_FIELD: &str = r#"
        /// The {relation_name} of the {parent_lower_case}, paginated
        async fn {relation_name}(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::{child_module}::{child}>> {
            super::{module}::resolve(ctx, self.id, offset, limit).await
        }"#;

fn relation_graphql_field(relation: &Relation) -> String {
    RELATION_GRAPHQL_FIELD
        .replace("{relation_name}", &relation.name)
        .replace("{parent_lower_case}", &relation.parent.to_lowercase())
        .replace("{child_module}", &relation.child_table)
        .replace("{child}", &relation.child)
        .replace("{module}", &relation.module())
}

// The parent model `source` with the field of the relation inserted after the marker of its `#[ComplexObject]`,
// unchanged when the field is already there, None without the marker (a model generated without --graphql)
fn with_relation_field(source: &str, relation: &Relation) -> Option<String> {
    let at = source.find(GRAPHQL_RELATIONS_MARKER)? + GRAPHQL_RELATIONS_MARKER.len();
    // `crate::` in the models generated before the sibling modules were imported with `super::`
    if ["super", "crate"].iter().any(|prefix| source.contains(&format!("{}::{}::resolve(", prefix, relation.module()))) {
        return Some(source.to_string());
    }
    Some(format!("{}{}{}", &source[..at], relation_graphql_field(relation), &source[at..]))
}

fn render_relation(relation: &Relation, openapi: bool, graphql: bool, sqlx: bool, timestamps: bool, dialect: Dialect) -> String {
    let (imports, derives, service_config, endpoint) = if openapi {
        (
            "\n    use apistos::ApiComponent;\n    use schemars::JsonSchema;\n    use octopux::gen_documented_relation_endpoint;",
            OPENAPI_DERIVES,
            "apistos::web::ServiceConfig",
            "gen_documented_relation_endpoint",
        )
    } else {
        (
            "\n    use octopux::gen_relation_endpoint;",
            "",
            "actix_web::web::ServiceConfig",
            "gen_relation_endpoint",
        )
    };
    let (body, query, state, limits) = if sqlx {
        (relation_sqlx_body(relation, timestamps, dialect), "query", "state", RELATION_LIMITS)
    } else {
        (
            "            // list the children of the parent `id`, None when it does not exist".to_string(),
            "_query",
            "_state",
            "",
        )
    };
    // A self-referencing relation (e.g. the subcategories of a category) already imports its child with the parent
    let child_import = if relation.child_table == relation.parent_table {
        String::new()
    } else {
        format!("\n    use super::{}::{};", relation.child_table, relation.child)
    };
    RELATION_TPL
        .replace("{child_import}", &child_import)
        .replace("{graphql_resolver}", if graphql { RELATION_GRAPHQL_RESOLVER } else { "" })
        .replace("{body}", &body)
        .replace("{query}", query)
        .replace("{state}", state)
        .replace("{list_limits}", limits)
        .replace("{openapi_imports}", imports)
        .replace("{openapi_derives}", derives)
        .replace("{service_config}", service_config)
        .replace("{endpoint}", endpoint)
        .replace("{relation}", &relation.type_name())
        .replace("{relation_name}", &relation.name)
        .replace("{module}", &relation.module())
        .replace("{parent_module}", &relation.parent_table)
        .replace("{child_module}", &relation.child_table)
        .replace("{parent_lower_case}", &relation.parent.to_lowercase())
        .replace("{parent}", &relation.parent)
        .replace("{child}", &relation.child)
}

// Index of the foreign key, which every page of the relation filters on
fn render_relation_migration(relation: &Relation, dialect: Dialect) -> String {
    let (table, column) = relation.foreign_key_column();
    let if_not_exists = if dialect == Dialect::Mysql { "" } else { "IF NOT EXISTS " };
    format!("CREATE INDEX {}{}_{}_idx ON {} ({});\n", if_not_exists, table, column, table, column)
}

const RELATION_TPL: &str = r#"
    // The application state, declared (or re-exported) at the root of the crate
    use crate::AppState;
    // The models and the relations are sibling modules, generated in the same folder
    use super::{parent_module}::{{parent}, Id};{child_import}
    use serde::Deserialize;
    use octopux::{
        HasMany,
        anyhow::Result,
        async_trait,
    };{openapi_imports}

    #[derive(Deserialize{openapi_derives})]
    pub struct {relation}Query {
        /// Number of rows to skip
        pub offset: Option<usize>,
        /// Maximum number of rows to return (20 by default, 100 at most)
        pub limit: Option<usize>,
    }{list_limits}

    /// The {relation_name} of a {parent_lower_case}, served on `GET /{parent_lower_case}/{id}/{relation_name}`
    pub struct {relation};

    #[async_trait]
    impl HasMany for {relation} {
        type Parent = {parent};
        type Id = Id;
        type Query = {relation}Query;
        type Result = Vec<{child}>;
        type State = AppState;
        const RELATION: &'static str = "{relation_name}";

        async fn list_related(id: Id, {query}: &{relation}Query, {state}: &AppState) -> Result<Option<Vec<{child}>>> {
{body}
        }
    }

    // Registers the route of the {relation_name} of a {parent_lower_case}, to mount with `.configure({module}::configure)`
    // in the same scope as the {parent_lower_case} routes
    pub fn configure(cfg: &mut {service_config}) {
        {endpoint}!({relation})(cfg)
    }{graphql_resolver}
    "#;

// Inserts the GraphQL field of the relation in the parent model, or explains how to add it
// when the parent model is missing or was generated without --graphql
fn add_relation_field(root: &Path, output: Option<&Path>, relation: &Relation) -> Result<(), Error> {
    let path = source_path(root, output, &format!("{}.rs", relation.parent_table));
    let source = fs::read_to_string(root.join(&path)).ok();
    match source.as_deref().and_then(|source| with_relation_field(source, relation).map(|patched| (source, patched))) {
        Some((source, patched)) if patched == source => {
            println!("{}", success(&format!("The GraphQL field `{}` is already declared in {}", relation.name, path)));
        }
        Some((_, patched)) => {
            fs::write(root.join(&path), patched)?;
            println!("{}", success(&format!("Added the GraphQL field `{}` to the {} type in {}", relation.name, relation.parent, path)));
        }
        None => {
            println!(
                "{}\n{}",
                warning(&format!("{} has no `#[ComplexObject]` of a model generated with --graphql, add this field to the `#[ComplexObject]` of {}:", path, relation.parent)),
                relation_graphql_field(relation)
            );
        }
    }
    Ok(())
}

// `source` with `root` appended to the tuple struct `name` (`struct Query(ApiQuery, ...);`),
// unchanged when it is already there, None when the struct is not declared
fn with_merged_root(source: &str, name: &str, root: &str) -> Option<String> {
    let start = source.find(&format!("\nstruct {}(", name))?;
    let end = start + source[start..].find(");")?;
    if source[start..end].split(|c: char| c == '(' || c == ',').any(|item| item.trim() == root) {
        return Some(source.to_string());
    }
    Some(format!("{}, {}{}", &source[..end], root, &source[end..]))
}

// The src/main.rs of --bootstrap --graphql `source` with the query and mutation roots of the model merged into
// the roots of the schema, the `EmptyMutation` of the bootstrap being replaced by the first mutation root,
// None without a `Query` root merged with MergedObject
fn with_graphql_roots(source: &str, module: &str, entity: &str) -> Option<String> {
    let query = format!("{}::{}Query", module, entity);
    let mutation = format!("{}::{}Mutation", module, entity);
    let source = with_merged_root(source, "Query", &query)?;
    if let Some(patched) = with_merged_root(&source, "Mutation", &mutation) {
        return Some(patched);
    }
    let empty = "type Mutation = EmptyMutation;";
    if !source.contains(empty) {
        return None;
    }
    Some(
        source
            .replace(empty, &format!("#[derive(MergedObject, Default)]\nstruct Mutation({});", mutation))
            .replace("{EmptyMutation, ", "{"),
    )
}

// Merges the query and mutation roots of the model into the schema of src/main.rs,
// printing them to merge by hand when src/main.rs was not bootstrapped with --graphql
fn add_graphql_roots(root: &Path, module: &str, entity: &str) -> Result<(), Error> {
    let path = source_path(root, None, "main.rs");
    let source = fs::read_to_string(root.join(&path)).ok();
    match source.as_deref().and_then(|source| with_graphql_roots(source, module, entity).map(|patched| (source, patched))) {
        Some((source, patched)) if patched == source => {
            println!("{}", success(&format!("The GraphQL roots of {} are already merged in {}", entity, path)));
        }
        Some((_, patched)) => {
            fs::write(root.join(&path), patched)?;
            println!("{}", success(&format!("Merged `{m}::{e}Query` and `{m}::{e}Mutation` into the roots of the schema in {p}", m = module, e = entity, p = path)));
        }
        None => {
            println!(
                "{}",
                highlight(&format!(
                    "  Merge `{module}::{name}Query` and `{module}::{name}Mutation` into the roots of the async-graphql schema, built with `.data(state.clone())`, its dependencies are added with `cargo add async-graphql@7 --features chrono` and `cargo add async-graphql-actix-web@7`",
                    module = module,
                    name = entity
                ))
            );
        }
    }
    Ok(())
}

// The structs of a model the fields are added to: `Book`, `NewBook` and `UpdatableBook`
fn model_struct_names(model: &str) -> [String; 3] {
    [model.to_string(), format!("New{}", model), format!("Updatable{}", model)]
}

// The structs of the model in a parsed model file, in the order of `model_struct_names`
fn model_structs<'a>(file: &'a syn::File, model: &str) -> Result<Vec<(&'a syn::ItemStruct, &'a syn::FieldsNamed)>, String> {
    model_struct_names(model)
        .iter()
        .map(|name| {
            let item = file.items.iter().find_map(|item| match item {
                syn::Item::Struct(s) if s.ident == name => Some(s),
                _ => None,
            });
            match item.map(|s| (s, &s.fields)) {
                Some((s, syn::Fields::Named(fields))) => Ok((s, fields)),
                Some(_) => Err(format!("`{}` is not a struct with named fields", name)),
                None => Err(format!("no `{}` struct", name)),
            }
        })
        .collect()
}

// The parsed model file `source`, or why it is not a model generated by octopux
fn parse_model_file(source: &str) -> Result<syn::File, String> {
    if !source.starts_with(generated_header("//").trim_end()) {
        return Err("it was not generated by octopux".to_string());
    }
    syn::parse_file(source).map_err(|e| format!("it does not parse ({})", e))
}

// What `add-field` reads in a model file
#[derive(Debug, PartialEq)]
struct ModelFile {
    // the fields of the model structs, the timestamps included
    fields: Vec<String>,
    // the model has `created_at`, `updated_at` and `deleted_at`
    timestamps: bool,
    // the model derives SqlxModel, whose queries take the new columns, the hand-written ones have to be updated
    sqlx: bool,
}

fn read_model_file(source: &str, model: &str) -> Result<ModelFile, String> {
    let file = parse_model_file(source)?;
    let structs = model_structs(&file, model)?;
    let mut fields: Vec<String> = Vec::new();
    for (_, named) in &structs {
        for ident in named.named.iter().filter_map(|f| f.ident.as_ref()) {
            let name = syn::ext::IdentExt::unraw(ident).to_string();
            if !fields.contains(&name) {
                fields.push(name);
            }
        }
    }
    let timestamps = TIMESTAMP_COLUMNS.iter().all(|c| fields.iter().any(|f| f == c));
    let sqlx = structs[0].0.attrs.iter().any(|attr| source[attr.span().byte_range()].contains("SqlxModel"));
    Ok(ModelFile { fields, timestamps, sqlx })
}

// The start of the line of `pos` and the indentation before `pos`, None when `pos` is not the first thing on its line
fn line_indent(source: &str, pos: usize) -> (usize, Option<&str>) {
    let start = source[..pos].rfind('\n').map_or(0, |i| i + 1);
    let prefix = &source[start..pos];
    (start, prefix.trim().is_empty().then_some(prefix))
}

// `source` with `fields` inserted in the model, creatable and updatable structs of `model`, before their timestamps,
// and the chrono types of the fields imported, as text so that the code and the comments of the file are kept
fn with_added_fields(source: &str, model: &str, fields: &[Field]) -> Result<String, String> {
    let file = parse_model_file(source)?;
    let mut edits: Vec<(usize, String)> = Vec::new();
    for (_, named) in model_structs(&file, model)? {
        let declaration = |indent: &str| fields.iter().map(|f| format!("{}pub {}: {},\n", indent, field_ident(&f.name), f.ty)).collect::<String>();
        let inline = || fields.iter().map(|f| format!("pub {}: {}, ", field_ident(&f.name), f.ty)).collect::<String>();
        let timestamp = named.named.iter().find(|f| f.ident.as_ref().is_some_and(|i| TIMESTAMP_COLUMNS.iter().any(|c| i == c)));
        match timestamp {
            Some(field) => {
                let pos = field.span().byte_range().start;
                match line_indent(source, pos) {
                    (start, Some(indent)) => edits.push((start, declaration(indent))),
                    (_, None) => edits.push((pos, inline())),
                }
            }
            None => {
                let close = named.brace_token.span.close().byte_range().start;
                if let Some(last) = named.named.last().filter(|_| !named.named.trailing_punct()) {
                    edits.push((last.span().byte_range().end, ",".to_string()));
                }
                match line_indent(source, close) {
                    (start, Some(indent)) => {
                        // the indentation of the last field, one level deeper than the brace without fields
                        let indent = match named.named.last().map(|f| line_indent(source, f.span().byte_range().start).1) {
                            Some(Some(field_indent)) => field_indent.to_string(),
                            _ => format!("{}    ", indent),
                        };
                        edits.push((start, declaration(&indent)));
                    }
                    (_, None) => edits.push((close, format!(" {}", inline()))),
                }
            }
        }
    }
    edits.extend(chrono_import_edit(source, &file, fields));
    // from the end, so that the offsets of the other edits stay valid
    edits.sort_by_key(|(pos, _)| std::cmp::Reverse(*pos));
    let mut patched = source.to_string();
    for (pos, text) in edits {
        patched.insert_str(pos, &text);
    }
    Ok(patched)
}

// The chrono names a use tree imports (`*` for a glob), and the braces of its `chrono::{...}` group
fn chrono_uses<'a>(tree: &'a syn::UseTree, in_chrono: bool, names: &mut Vec<String>, group: &mut Option<&'a syn::UseGroup>) {
    match tree {
        syn::UseTree::Path(path) => {
            let chrono = !in_chrono && path.ident == "chrono";
            if let (true, syn::UseTree::Group(g)) = (chrono, path.tree.as_ref()) {
                *group = Some(g);
            }
            chrono_uses(&path.tree, in_chrono || chrono, names, group);
        }
        syn::UseTree::Name(name) if in_chrono => names.push(name.ident.to_string()),
        syn::UseTree::Rename(rename) if in_chrono => names.push(rename.rename.to_string()),
        syn::UseTree::Glob(_) if in_chrono => names.push("*".to_string()),
        syn::UseTree::Group(g) => g.items.iter().for_each(|t| chrono_uses(t, in_chrono, names, group)),
        _ => {}
    }
}

// The edit importing the chrono types of `fields` the file does not import yet, added to its `use chrono::{...};`,
// or in a new one after the last `use`
fn chrono_import_edit(source: &str, file: &syn::File, fields: &[Field]) -> Option<(usize, String)> {
    let uses: Vec<&syn::ItemUse> = file.items.iter().filter_map(|item| match item {
        syn::Item::Use(u) => Some(u),
        _ => None,
    }).collect();
    let (mut imported, mut group) = (Vec::new(), None);
    for item in &uses {
        chrono_uses(&item.tree, false, &mut imported, &mut group);
    }
    let missing: Vec<&str> = chrono_names(fields, false).into_iter().filter(|n| !imported.iter().any(|i| i == n || i == "*")).collect();
    if missing.is_empty() {
        return None;
    }
    match (group, uses.last()) {
        (Some(group), _) => {
            let separator = if group.items.is_empty() || group.items.trailing_punct() { "" } else { ", " };
            Some((group.brace_token.span.close().byte_range().start, format!("{}{}", separator, missing.join(", "))))
        }
        (None, Some(last)) => {
            let indent = line_indent(source, last.span().byte_range().start).1.unwrap_or_default();
            Some((last.semi_token.span().byte_range().end, format!("\n{}use chrono::{{{}}};", indent, missing.join(", "))))
        }
        (None, None) => {
            let first = file.items.first()?.span().byte_range().start;
            let (start, indent) = line_indent(source, first);
            Some((start, format!("{}use chrono::{{{}}};\n", indent.unwrap_or_default(), missing.join(", "))))
        }
    }
}

// Adds fields to an existing model and, with --migration, writes the migration adding their columns
#[allow(clippy::too_many_arguments)]
fn add_fields<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    model: &str,
    table: &str,
    path: &str,
    migration: bool,
    tables: Option<&[Table]>,
    unique: bool,
    default: Option<&str>,
    dialect: Dialect,
) -> Result<(), Error> {
    let fail = |message: String| -> ! {
        eprintln!("{}", failure(&message));
        process::exit(1);
    };
    let source = fs::read_to_string(path).unwrap_or_else(|e| fail(format!("{} not read ({}), no field added", path, e)));
    let declared = read_model_file(&source, model).unwrap_or_else(|e| fail(format!("{} is not a model of {}: {}, no field added", path, model, e)));
    let strict = migration.then_some(dialect);
    let references = tables.map(|t| (table, t));
    let mut fields = read_new_fields(input, output, declared.timestamps, &declared.fields, strict, references, unique)?;
    if fields.is_empty() {
        fail(format!("No field entered, {} unchanged", path));
    }
    let mut defaults = Vec::new();
    if migration {
        for field in fields.iter_mut() {
            if let Some(d) = read_default(input, output, field, default, dialect)? {
                defaults.push((field.name.clone(), d));
            }
            let field_default = defaults.iter().find(|(n, _)| *n == field.name).map(|(_, d)| d.as_str());
            if let Some(error) = add_column_error(field, field_default, dialect) {
                fail(format!("{}, no field added", error));
            }
            if field.unique && field_default.is_some() {
                writeln!(output, "{}", warning(&format!("The existing rows all get the default of `{}`, its unique index is refused when the table has several rows", field.name)))?;
            }
        }
    }
    let patched = with_added_fields(&source, model, &fields).unwrap_or_else(|e| fail(format!("{} not patched: {}", path, e)));
    let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
    let migration = if migration {
        let sql = render_add_columns(table, &fields, &defaults, dialect).unwrap_or_else(|e| fail(format!("{}, no field added", e)));
        Some((migration_path(&format!("add_{}_to_{}", names.join("_"), table))?, sql))
    } else {
        None
    };
    let mut files = vec![(path.to_string(), true)];
    files.extend(migration.iter().map(|(p, _)| (p.display().to_string(), false)));
    if !confirm_save(input, output, &files)? {
        fail(format!("Nothing saved, no field added to {}", model));
    }
    fs::write(path, patched)?;
    let [_, new, updatable] = model_struct_names(model);
    writeln!(output, "{}", success(&format!("Added {} to {}, {} and {} in {}", names.join(", "), model, new, updatable, path)))?;
    if !declared.sqlx {
        writeln!(output, "{}", warning(&format!("{} does not derive SqlxModel, add the columns to the queries of its model functions", model)))?;
    }
    if let Some((migration, sql)) = migration {
        write_migration(&migration, &sql)?;
    }
    Ok(())
}

// Writes src/main.rs and src/helpers.rs under `root`, only if src/helpers.rs does not exist,
// an existing src/main.rs (such as the one of `cargo init`) is only overwritten once confirmed
fn bootstrap<R: BufRead, W: Write>(root: &Path, openapi: bool, graphql: bool, dialect: Dialect, input: &mut R, output: &mut W) -> Result<(), Error> {
    let main_content = render_bootstrap_main(openapi, graphql, dialect);
    let helpers_content = render_bootstrap_helpers(dialect);
    let files = [("main.rs", main_content.as_str()), ("helpers.rs", helpers_content.as_str())];
    let src = root.join("src");
    let helpers = src.join("helpers.rs");
    if helpers.exists() {
        eprintln!("{}", failure(&format!("{} already exists, project not bootstrapped", helpers.display())));
        process::exit(1);
    }
    let main = src.join("main.rs");
    if main.exists() && !confirm(input, output, &format!("{} already exists, overwrite it with the generated one?", main.display()))? {
        eprintln!("{}", failure(&format!("{} kept, project not bootstrapped", main.display())));
        process::exit(1);
    }
    fs::create_dir_all(&src)?;
    for (name, content) in files {
        fs::write(src.join(name), with_header("//", content))?;
    }
    println!(
        "{}",
        success(&format!(
            "Successfully bootstrapped src/main.rs and src/helpers.rs, generate a model with `octopux generate-model --name <Model>{}{}{}`, then declare it with `mod <model>;` and mount it with `.configure(<model>::configure)` in the v1 scope of src/main.rs",
            if openapi { " --openapi" } else { "" },
            if graphql { " --graphql --fields" } else { "" },
            format!(" {}", dialect.flag())
        ))
    );
    if dialect != Dialect::Sqlite {
        println!("{}", highlight(&format!("  src/main.rs connects to the {} database of `DATABASE_URL`", dialect.name())));
    }
    if graphql {
        println!("{}", highlight("  Its `<Model>Query` and `<Model>Mutation` are merged into the `Query` and `Mutation` roots of src/main.rs, GraphiQL is served on GET /graphql"));
    }
    Ok(())
}

// `cargo add` arguments of the dependencies of the bootstrapped files and of the generated models,
// octopux is taken from the tag of the CLI version in its repository, so that the generated code matches its macros,
// apistos and schemars are only added with --openapi, async-graphql and its actix integration with --graphql
fn bootstrap_dependencies(openapi: bool, graphql: bool, dialect: Dialect) -> Vec<Vec<String>> {
    let tag = format!("v{}", env!("CARGO_PKG_VERSION"));
    let features = if openapi { "openapi,sqlx" } else { "sqlx" };
    let mut deps = vec![
        vec!["octopux", "--git", env!("CARGO_PKG_REPOSITORY"), "--tag", tag.as_str(), "--features", features],
        vec!["actix-web@4"],
    ];
    if openapi {
        deps.push(vec!["apistos@0.9", "--features", "chrono,swagger-ui"]);
        deps.push(vec!["apistos-schemars@0.8", "--rename", "schemars"]);
    }
    if graphql {
        // the chrono scalars, not enabled by default
        deps.push(vec!["async-graphql@7", "--features", "chrono"]);
        deps.push(vec!["async-graphql-actix-web@7"]);
    }
    let sqlx_features = format!(
        "runtime-tokio,{},chrono,macros,migrate",
        match dialect {
            Dialect::Sqlite => "sqlite",
            Dialect::Postgres => "postgres",
            Dialect::Mysql => "mysql",
        }
    );
    deps.extend([
        vec!["serde@1", "--features", "derive"],
        vec!["chrono@0.4", "--features", "serde"],
        vec!["sqlx@0.9", "--no-default-features", "--features", sqlx_features.as_str()],
    ]);
    deps.iter().map(|args| args.iter().map(|a| a.to_string()).collect()).collect()
}

// Options of --bootstrap: the database, --openapi and --graphql
#[derive(Debug, PartialEq)]
struct BootstrapOptions {
    dialect: Dialect,
    openapi: bool,
    graphql: bool,
}

const DIALECTS: [Dialect; 3] = [Dialect::Sqlite, Dialect::Postgres, Dialect::Mysql];

// With a database flag, the flags are taken as is so that --bootstrap stays scriptable,
// otherwise asks for the database, then for OpenAPI and GraphQL unless their flag is given,
// and prints the equivalent command
fn bootstrap_options<R: BufRead, W: Write>(cli: &Cli, input: &mut R, output: &mut W) -> Result<BootstrapOptions, Error> {
    if cli.sqlite || cli.postgres || cli.mysql {
        return Ok(BootstrapOptions { dialect: Dialect::from_flags(cli.postgres, cli.mysql), openapi: cli.openapi, graphql: cli.graphql });
    }
    writeln!(output, "{}", bold("Bootstrap"))?;
    let menu = DIALECTS
        .iter()
        .enumerate()
        .map(|(i, dialect)| format!("{} {}", magenta(&format!("{})", i + 1)), dialect.name()))
        .collect::<Vec<_>>()
        .join("  ");
    writeln!(output, "  {}", menu)?;
    let dialect = loop {
        let message = format!("{} {} {} ", cyan("?"), bold("Database ›"), dim(&format!("[{}]", DIALECTS[0].name())));
        let answer = prompt(input, output, &message)?.unwrap_or_default();
        let chosen = if answer.is_empty() {
            Some(DIALECTS[0])
        } else {
            answer
                .parse::<usize>()
                .ok()
                .and_then(|n| DIALECTS.get(n.wrapping_sub(1)).copied())
                .or_else(|| DIALECTS.iter().copied().find(|d| d.name().eq_ignore_ascii_case(&answer) || d.flag() == format!("--{}", answer.to_lowercase())))
        };
        match chosen {
            Some(dialect) => break dialect,
            None => writeln!(output, "{}", failure(&format!("`{}` is not a database, enter a number between 1 and {}", answer, DIALECTS.len())))?,
        }
    };
    let openapi = cli.openapi || confirm(input, output, "Document the routes with OpenAPI and Swagger UI (apistos)?")?;
    let graphql = cli.graphql || confirm(input, output, "Serve a GraphQL schema and GraphiQL on /graphql (async-graphql)?")?;
    writeln!(
        output,
        "\n{}",
        dim(&highlight(&format!(
            "Next time, skip the questions with `octopux --bootstrap {}{}{}`",
            dialect.flag(),
            if openapi { " --openapi" } else { "" },
            if graphql { " --graphql" } else { "" }
        )))
    )?;
    Ok(BootstrapOptions { dialect, openapi, graphql })
}

// Only an explicit yes accepts, an empty answer or the end of the input refuses
fn confirm<R: BufRead, W: Write>(input: &mut R, output: &mut W, message: &str) -> Result<bool, Error> {
    let answer = prompt(input, output, &format!("{} {} {} ", cyan("?"), bold(&highlight(message)), dim("(y/N)")))?;
    Ok(matches!(answer.as_deref().map(str::to_lowercase).as_deref(), Some("y" | "yes")))
}

// Runs `cargo add` in `root` for each dependency of the bootstrapped project
fn install_dependencies(root: &Path, openapi: bool, graphql: bool, dialect: Dialect) -> Result<(), Error> {
    if !root.join("Cargo.toml").exists() {
        eprintln!("{}", failure(&format!("No Cargo.toml in {}, dependencies not installed, create the crate with `cargo init` first", root.display())));
        process::exit(1);
    }
    for args in bootstrap_dependencies(openapi, graphql, dialect) {
        let status = process::Command::new("cargo").arg("add").args(&args).current_dir(root).status()?;
        if !status.success() {
            eprintln!("{}", failure(&format!("`cargo add {}` failed, remaining dependencies not installed", args.join(" "))));
            process::exit(1);
        }
    }
    println!("{}", success("Successfully installed the dependencies"));
    Ok(())
}

fn main() -> Result<(), Error> {
    let cli = Cli::from_args();
    COLOR.store(
        io::stdout().is_terminal() && io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
        Ordering::Relaxed,
    );
    if cli.bootstrap {
        let root = std::env::current_dir()?;
        let mut input = io::stdin().lock();
        let BootstrapOptions { dialect, openapi, graphql } = bootstrap_options(&cli, &mut input, &mut io::stdout())?;
        bootstrap(&root, openapi, graphql, dialect, &mut input, &mut io::stdout())?;
        if confirm(&mut input, &mut io::stdout(), "Install the dependencies with `cargo add`?")? {
            install_dependencies(&root, openapi, graphql, dialect)?;
        }
    }
    match cli.cmd {
        Some(opt) => run(opt),
        None if cli.bootstrap => Ok(()),
        None => {
            Cli::clap().print_help().map_err(|e| Error::other(e.to_string()))?;
            println!();
            Ok(())
        }
    }
}

fn run(opt: Opt) -> Result<(), Error> {
    match opt {
        Opt::GenerateModel { name, openapi, graphql, fields, sqlx, migration, foreign_keys, unique, table, sqlite: _, postgres, mysql, timestamps, force, output } => {
            let dialect = Dialect::from_flags(postgres, mysql);
            if let Some(invalid) = table.as_ref().filter(|t| !is_field_name(t)) {
                eprintln!("{}", failure(&format!("`{}` is not a valid table name, use snake_case, model {} not generated", invalid, name)));
                process::exit(1);
            }
            let table = table.unwrap_or_else(|| to_snake_case(&name));
            // the file and the module are named after the table
            let module = table.clone();
            let path = source_path(&std::env::current_dir()?, output.as_deref(), &format!("{}.rs", module));
            let overwrites = Path::new(&path).exists();
            refuse_overwrite(&path, force, &format!("model {}", name));
            let fields_asked = fields;
            let fields = if fields {
                let strict = if migration { Some(dialect) } else { None };
                let tables = if foreign_keys { known_tables(dialect)? } else { Vec::new() };
                let references = foreign_keys.then_some((table.as_str(), tables.as_slice()));
                read_fields(&mut io::stdin().lock(), &mut io::stdout(), timestamps, strict, references, unique)?
            } else {
                Vec::new()
            };
            // the INSERT and UPDATE queries need at least one column
            if sqlx && fields.is_empty() {
                eprintln!("{}", failure(&format!("--sqlx needs at least one field, model {} not generated", name)));
                process::exit(1);
            }
            // GraphQL refuses an input object without fields, as the creatable struct would be
            if graphql && fields.is_empty() {
                eprintln!("{}", failure(&format!("--graphql needs at least one field, model {} not generated", name)));
                process::exit(1);
            }
            let sql = if migration {
                Some(render_migration(&table, &fields, timestamps, dialect).unwrap_or_else(|e| {
                    eprintln!("{}", failure(&format!("{}, model {} not generated", e, name)));
                    process::exit(1);
                }))
            } else {
                None
            };
            let migration = match sql {
                Some(sql) => Some((migration_path(&format!("create_{}", table))?, sql)),
                None => None,
            };
            // the fields were entered interactively, nothing is written before the recap is confirmed
            if fields_asked {
                let mut files = vec![(path.clone(), overwrites)];
                files.extend(migration.iter().map(|(p, _)| (p.display().to_string(), false)));
                if !confirm_save(&mut io::stdin().lock(), &mut io::stdout(), &files)? {
                    eprintln!("{}", failure(&format!("Nothing saved, model {} not generated", name)));
                    process::exit(1);
                }
            }
            write_source(&path, &render_table_model(&name, Some(&table), openapi, graphql, sqlx, timestamps, &fields, dialect))?;
            println!("{}", success(&format!("Successfully generated model {}, declare it with `mod {};`", path, module)));
            if graphql {
                add_graphql_roots(&std::env::current_dir()?, &module, &name)?;
            }
            if let Some((migration, sql)) = migration {
                write_migration(&migration, &sql)?;
                if overwrites {
                    println!(
                        "{}",
                        warning(&format!("The migration creates the {} table only if it does not exist yet", table))
                    );
                }
            }
            Ok(())
        }
        Opt::GenerateRelation { parent, child, name, foreign_key, through, child_key, parent_table, child_table, through_table, openapi, graphql, sqlx, migration, sqlite: _, postgres, mysql, timestamps, force, output } => {
            let dialect = Dialect::from_flags(postgres, mysql);
            let relation = Relation::new(parent, child, name, foreign_key, through, child_key).with_tables(parent_table, child_table, through_table);
            let columns = [
                Some(&relation.name),
                Some(&relation.foreign_key),
                relation.through.as_ref().map(|t| &t.child_key),
                Some(&relation.parent_table),
                Some(&relation.child_table),
                relation.through.as_ref().map(|t| &t.table),
            ];
            if let Some(invalid) = columns.into_iter().flatten().find(|c| !is_field_name(c)) {
                eprintln!("{}", failure(&format!("`{}` is not a valid name, use snake_case, relation not generated", invalid)));
                process::exit(1);
            }
            let module = relation.module();
            let path = source_path(&std::env::current_dir()?, output.as_deref(), &format!("{}.rs", module));
            refuse_overwrite(&path, force, "relation");
            write_source(&path, &render_relation(&relation, openapi, graphql, sqlx, timestamps, dialect))?;
            println!(
                "{}",
                success(&format!(
                    "Successfully generated relation {}, declare it with `mod {};` and mount it with `.configure({}::configure)` in the scope of the {} routes",
                    path, module, module, relation.parent.to_lowercase()
                ))
            );
            if graphql {
                add_relation_field(&std::env::current_dir()?, output.as_deref(), &relation)?;
            }
            if migration {
                let (table, column) = relation.foreign_key_column();
                write_migration(&migration_path(&format!("index_{}_{}", table, column))?, &render_relation_migration(&relation, dialect))?;
            }
            Ok(())
        }
        Opt::AddField { model, migration, foreign_keys, unique, default, table, sqlite: _, postgres, mysql, output } => {
            let dialect = Dialect::from_flags(postgres, mysql);
            if let Some(invalid) = table.as_ref().filter(|t| !is_field_name(t)) {
                eprintln!("{}", failure(&format!("`{}` is not a valid table name, use snake_case, no field added", invalid)));
                process::exit(1);
            }
            let table = table.unwrap_or_else(|| to_snake_case(&model));
            let path = source_path(&std::env::current_dir()?, output.as_deref(), &format!("{}.rs", table));
            let tables = if foreign_keys { Some(known_tables(dialect)?) } else { None };
            let mut input = io::stdin().lock();
            add_fields(&mut input, &mut io::stdout(), &model, &table, &path, migration, tables.as_deref(), unique, default.as_deref(), dialect)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        migration_timestamp, migrations_dir, source_path, parse_field_type, pluralize, read_fields, render_migration, render_model, render_table_model, render_relation, render_relation_migration, to_camel_case,
        to_snake_case, bootstrap, bootstrap_dependencies, bootstrap_options, BootstrapOptions, render_bootstrap_main, changes_summary, confirm, confirm_save, generated_header, migration_tables, parse_tables, postgres_column_type, url_dialect,
        Cli, Column, Dialect, Field, Opt, Reference, Relation, Table, Through, LOGO,
        add_column_error, read_default, read_model_file, read_new_fields, render_add_columns, with_added_fields, write_source, ModelFile,
    };
    use structopt::StructOpt;

    fn field(name: &str, ty: &str) -> Field {
        Field { name: name.into(), ty: ty.into(), references: None, unique: false, length: None }
    }

    fn column(name: &str, sql_type: &str, unique: bool) -> Column {
        Column { name: name.into(), sql_type: sql_type.into(), unique }
    }

    fn author_table() -> Table {
        Table { name: "author".into(), columns: vec![column("id", "INT8", true), column("email", "TEXT", true), column("name", "TEXT", false)] }
    }

    #[test]
    fn parse_tables_reads_the_created_tables() {
        let sql = super::with_header("--", "CREATE TABLE IF NOT EXISTS author (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    score DOUBLE PRECISION,
    price DECIMAL(10, 2) DEFAULT 0,
    email VARCHAR(255) UNIQUE NOT NULL
);
create table \"Book\" (code TEXT, author_id INT8, PRIMARY KEY (code), FOREIGN KEY (author_id) REFERENCES author (id));
CREATE INDEX book_idx ON book (code);
");
        assert_eq!(parse_tables(&sql), vec![
            Table {
                name: "author".into(),
                columns: vec![
                    column("id", "BIGSERIAL", true),
                    column("name", "TEXT", false),
                    column("score", "DOUBLE PRECISION", false),
                    column("price", "DECIMAL(10, 2)", false),
                    column("email", "VARCHAR(255)", true),
                ],
            },
            Table { name: "Book".into(), columns: vec![column("code", "TEXT", true), column("author_id", "INT8", false)] },
        ]);
    }

    #[test]
    fn migration_tables_keep_the_first_definition_of_a_table() {
        let dir = std::env::temp_dir().join(format!("octopux-migration-tables-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("2_create_book.sql"), "CREATE TABLE IF NOT EXISTS book (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL);").unwrap();
        std::fs::write(dir.join("1_create_book.sql"), "CREATE TABLE IF NOT EXISTS book (id INTEGER PRIMARY KEY AUTOINCREMENT);").unwrap();
        std::fs::write(dir.join("3_create_author.down.sql"), "CREATE TABLE author (id INTEGER);").unwrap();
        let tables = migration_tables(&dir);
        assert_eq!(tables, vec![Table { name: "book".into(), columns: vec![column("id", "INTEGER", true)] }]);
        assert!(migration_tables(&dir.join("missing")).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn read_fields_asks_for_the_referenced_table_and_column() {
        let tables = [author_table()];
        // table by number and default column, table and column by name, no reference, unknown table then self reference
        let input = "author_id:i64\n1\n\nwriter_email\n\n100\nAUTHOR\nemail\ntitle\n\n\n\nparent_id:i64\nnope\n2\n\n\n";
        let mut output = Vec::new();
        let fields = read_fields(&mut input.as_bytes(), &mut output, false, Some(Dialect::Postgres), Some(("book", &tables)), false).unwrap();
        let reference = |table: &str, column: &str| Some(Reference { table: table.into(), column: column.into() });
        assert_eq!(fields, vec![
            Field { references: reference("author", "id"), ..field("author_id", "i64") },
            Field { references: reference("author", "email"), length: Some(100), ..field("writer_email", "String") },
            field("title", "String"),
            Field { references: reference("book", "id"), ..field("parent_id", "i64") },
        ]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("1) author  2) book (this model)"));
        assert!(output.contains("1) id INT8 (unique)  2) email TEXT (unique)  3) name TEXT"));
        assert!(output.contains("? Column of `author` referenced by `author_id` › (number or name) [id] "));
        assert!(output.contains("`nope` is not a known table, pick 1 to 2"));
        // the model table has its id and the fields declared before
        assert!(output.contains("1) id BIGSERIAL (unique)  2) author_id INT8  3) writer_email VARCHAR(100)  4) title VARCHAR(255)"));
        assert!(output.contains("writer_email: String (length 100) → author (email)"));
        assert!(output.contains("author_id: i64 → author (id)"));
        assert!(!output.contains("may be refused"));
    }

    #[test]
    fn read_fields_warns_about_references_the_database_refuses() {
        let tables = [author_table()];
        let mut output = Vec::new();
        let input = "author_id:i32\nauthor\n\nauthor_name:String\n\n1\nname\n\n";
        let fields = read_fields(&mut input.as_bytes(), &mut output, false, Some(Dialect::Postgres), Some(("book", &tables)), false).unwrap();
        assert_eq!(fields.len(), 2);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("`author_id` is INT4 and `author.id` is INT8, the foreign key may be refused"));
        assert!(output.contains("`author.name` is neither a primary key nor unique"));
        // SQLite does not check the types
        let mut output = Vec::new();
        read_fields(&mut "author_id:i32\n1\n\n\n".as_bytes(), &mut output, false, Some(Dialect::Sqlite), Some(("book", &tables)), false).unwrap();
        assert!(!String::from_utf8(output).unwrap().contains("may be refused"));
    }

    #[test]
    fn migration_declares_the_foreign_keys() {
        let fields = vec![
            Field { references: Some(Reference { table: "author".into(), column: "id".into() }), ..field("author_id", "i64") },
            field("title", "String"),
            Field { references: Some(Reference { table: "book".into(), column: "id".into() }), ..field("parent_id", "Option<i64>") },
        ];
        assert_eq!(render_migration("Book", &fields, true, Dialect::Mysql).unwrap(), "CREATE TABLE IF NOT EXISTS book (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    author_id BIGINT NOT NULL,
    title VARCHAR(255) NOT NULL,
    parent_id BIGINT,
    created_at DATETIME(6),
    updated_at DATETIME(6),
    deleted_at DATETIME(6),
    CONSTRAINT fk_book_author_id FOREIGN KEY (author_id) REFERENCES author (id),
    CONSTRAINT fk_book_parent_id FOREIGN KEY (parent_id) REFERENCES book (id)
);
");
    }

    #[test]
    fn foreign_keys_flag_requires_migration() {
        let parse = |flags: &[&str]| {
            let mut args = vec!["octopux", "generate-model", "--name", "Book", "--sqlite", "--fields"];
            args.extend(flags);
            Opt::from_iter_safe(&args)
        };
        assert!(parse(&["--foreign-keys"]).is_err());
        assert!(parse(&["--foreign-keys", "--migration"]).is_ok());
    }

    #[test]
    fn read_fields_asks_whether_the_columns_are_unique() {
        // no reference and unique, not unique by default, then a self reference proposing the unique field as unique
        let input = "email\n\n\n\ny\nname\n\n\n\n\nparent_email\n\n\n1\nemail\nno\n\n";
        let mut output = Vec::new();
        let fields = read_fields(&mut input.as_bytes(), &mut output, false, Some(Dialect::Postgres), Some(("author", &[])), true).unwrap();
        let unique = |name: &str| Field { unique: true, ..field(name, "String") };
        assert_eq!(fields, vec![
            unique("email"),
            field("name", "String"),
            Field { references: Some(Reference { table: "author".into(), column: "email".into() }), ..field("parent_email", "String") },
        ]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("? Is `email` unique › (y/N) "));
        assert!(output.contains("1) id BIGSERIAL (unique)  2) email VARCHAR(255) (unique)  3) name VARCHAR(255)"));
        assert!(output.contains("email: String (unique)"));
        assert!(!output.contains("neither a primary key nor unique"));
    }

    #[test]
    fn read_fields_warns_about_unique_columns_mysql_refuses() {
        let mut output = Vec::new();
        let fields = read_fields(&mut "hash:Vec<u8>\ny\nemail\n\n\ny\n\n".as_bytes(), &mut output, false, Some(Dialect::Mysql), None, true).unwrap();
        assert!(fields.iter().all(|f| f.unique));
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("MySQL refuses a unique index on the BLOB column `hash`"));
        assert!(!output.contains("column `email`"));
    }

    #[test]
    fn read_fields_asks_for_the_varchar_lengths() {
        // too long for MySQL, zero, then a length, the default length, no length asked for a TEXT column
        let input = "title\n\n20000\n0\n80\nsummary:String?\n\n\n";
        let mut output = Vec::new();
        let fields = read_fields(&mut input.as_bytes(), &mut output, false, Some(Dialect::Mysql), None, false).unwrap();
        assert_eq!(fields, vec![Field { length: Some(80), ..field("title", "String") }, field("summary", "Option<String>")]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("? Length of `title` › (VARCHAR, 1 to 16383) [255] "));
        assert!(output.contains("`20000` is not a MySQL VARCHAR length, pick 1 to 16383"));
        assert!(output.contains("`0` is not a MySQL VARCHAR length"));
        assert_eq!(render_migration("Post", &fields, false, Dialect::Mysql).unwrap(), "CREATE TABLE IF NOT EXISTS post (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    title VARCHAR(80) NOT NULL,
    summary VARCHAR(255)
);
");
        let mut output = Vec::new();
        read_fields(&mut "title\n\n\n".as_bytes(), &mut output, false, Some(Dialect::Sqlite), None, false).unwrap();
        assert!(!String::from_utf8(output).unwrap().contains("Length of"));
    }

    #[test]
    fn migration_declares_the_unique_constraints() {
        let fields = vec![
            Field { unique: true, ..field("email", "String") },
            field("title", "String"),
            Field { unique: true, references: Some(Reference { table: "author".into(), column: "id".into() }), ..field("author_id", "i64") },
        ];
        assert_eq!(render_migration("Book", &fields, false, Dialect::Postgres).unwrap(), "CREATE TABLE IF NOT EXISTS book (
    id BIGSERIAL PRIMARY KEY,
    email VARCHAR(255) NOT NULL,
    title VARCHAR(255) NOT NULL,
    author_id INT8 NOT NULL,
    UNIQUE (email),
    UNIQUE (author_id),
    CONSTRAINT fk_book_author_id FOREIGN KEY (author_id) REFERENCES author (id)
);
");
    }

    #[test]
    fn unique_flag_requires_migration() {
        let parse = |flags: &[&str]| {
            let mut args = vec!["octopux", "generate-model", "--name", "Book", "--sqlite", "--fields"];
            args.extend(flags);
            Opt::from_iter_safe(&args)
        };
        assert!(parse(&["--unique"]).is_err());
        assert!(parse(&["--unique", "--migration"]).is_ok());
    }

    #[test]
    fn database_url_types_match_the_sqlx_types() {
        assert_eq!(url_dialect("postgres://user@localhost/db"), Some(Dialect::Postgres));
        assert_eq!(url_dialect("sqlite://data.db?mode=rwc"), Some(Dialect::Sqlite));
        assert_eq!(url_dialect("mariadb://localhost/db"), Some(Dialect::Mysql));
        assert_eq!(url_dialect("redis://localhost"), None);
        assert_eq!(postgres_column_type("int8"), "INT8");
        assert_eq!(postgres_column_type("_text"), "TEXT[]");
        assert!(Dialect::Postgres.same_column_type("INT8", "bigserial"));
        assert!(Dialect::Postgres.same_column_type("FLOAT8", "DOUBLE  PRECISION"));
        assert!(!Dialect::Postgres.same_column_type("INT4", "INT8"));
        assert!(Dialect::Postgres.same_column_type("VARCHAR(255)", "text"));
        assert!(Dialect::Postgres.same_column_type("VARCHAR(255)", "VARCHAR"));
        assert!(!Dialect::Postgres.same_column_type("VARCHAR(255)", "INT8"));
        assert!(Dialect::Mysql.same_column_type("BIGINT", "bigint"));
        assert!(!Dialect::Mysql.same_column_type("INT", "BIGINT"));
    }

    #[test]
    fn default_model_has_no_openapi_derives_and_an_undocumented_configure() {
        let model = render_model("Project", false, false, false, false, &[], Dialect::Sqlite);
        assert!(!model.contains("JsonSchema"));
        assert!(!model.contains("ApiComponent"));
        assert!(model.contains("use octopux::gen_endpoint;"));
        assert!(model.contains("pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {\n        gen_endpoint!(Project, NewProject, UpdatableProject)(cfg)\n    }"));
        assert!(model.contains("`.configure(project::configure)`"));
        assert!(!model.contains("gen_documented_endpoint"));
        assert!(!model.contains("{openapi"));
        assert!(model.contains("#[octopux_info(path = \"project\")]"));
    }

    #[test]
    fn openapi_model_derives_schemas_on_every_route_type() {
        let model = render_model("Project", true, false, false, false, &[], Dialect::Sqlite);
        assert!(model.contains("use apistos::ApiComponent;"));
        assert!(model.contains("use schemars::JsonSchema;"));
        assert!(model.contains("use octopux::gen_documented_endpoint;"));
        assert!(model.contains("pub fn configure(cfg: &mut apistos::web::ServiceConfig) {"));
        assert!(model.contains("gen_documented_endpoint!(Project, NewProject, UpdatableProject)(cfg)"));
        // 5 query structs + Project, NewProject and UpdatableProject
        assert_eq!(model.matches(", JsonSchema, ApiComponent").count(), 8);
        assert!(!model.contains("{openapi"));
        assert!(!model.contains("{entity"));
    }

    #[test]
    fn model_only_depends_on_octopux_and_the_app_state() {
        let model = render_model("Project", false, false, false, false, &[], Dialect::Sqlite);
        assert!(model.contains("use crate::AppState;"));
        assert!(model.contains("        octopux_info,\n        anyhow::Result,\n        async_trait,\n    };"));
        assert!(!model.contains("octopux_derive"));
        assert!(!model.contains("use anyhow"));
        assert!(!model.contains("use async_trait"));
    }

    #[test]
    fn model_without_fields_keeps_empty_structs() {
        let model = render_model("Project", false, false, false, false, &[], Dialect::Sqlite);
        assert!(model.contains("pub type Id = i64;"));
        assert!(model.contains("pub struct Project {\n        pub id: Id,\n    }"));
        assert!(model.contains("pub struct NewProject {\n\n    }"));
        assert!(model.contains("pub struct UpdatableProject {\n        pub id: Id,\n    }"));
        assert!(!model.contains("_fields}"));
    }

    #[test]
    fn fields_are_added_to_every_model_struct() {
        let fields = vec![
            field("title", "String"),
            field("stars", "i32"),
        ];
        let model = render_model("Project", false, false, false, false, &fields, Dialect::Sqlite);
        assert!(model.contains("pub struct Project {\n        pub id: Id,\n        pub title: String,\n        pub stars: i32,\n    }"));
        assert!(model.contains("pub struct NewProject {\n        pub title: String,\n        pub stars: i32,\n    }"));
        assert!(model.contains("pub struct UpdatableProject {\n        pub id: Id,\n        pub title: String,\n        pub stars: i32,\n    }"));
    }

    #[test]
    fn read_fields_prompts_until_empty_name() {
        // invalid names, default type, duplicate and reserved `id` are handled
        let input = "title\n\n1bad\nstars\ni32\ntitle\nid\n\n";
        let mut output = Vec::new();
        let fields = read_fields(&mut input.as_bytes(), &mut output, false, None, None, false).unwrap();
        assert_eq!(fields, vec![
            field("title", "String"),
            field("stars", "i32"),
        ]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("`1bad` is not a valid field name"));
        assert!(output.contains("Field `title` is already declared"));
        assert!(output.contains("Field `id` is already declared"));
    }

    #[test]
    fn read_fields_converts_names_to_snake_case() {
        let input = "OptStr\n\nhttpCode\n\nfirst name\n\nlast-name\n\nopt_str\nID\n\n";
        let mut output = Vec::new();
        let fields = read_fields(&mut input.as_bytes(), &mut output, false, None, None, false).unwrap();
        let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["opt_str", "http_code", "first_name", "last_name"]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("`OptStr` renamed to `opt_str`"));
        assert!(output.contains("Field `opt_str` is already declared"));
        assert!(output.contains("Field `id` is already declared"));
    }

    #[test]
    fn to_snake_case_splits_words_and_acronyms() {
        assert_eq!(to_snake_case("title"), "title");
        assert_eq!(to_snake_case("createdAt"), "created_at");
        assert_eq!(to_snake_case("CreatedAt"), "created_at");
        assert_eq!(to_snake_case("HTTPCode"), "http_code");
        assert_eq!(to_snake_case("userID"), "user_id");
        assert_eq!(to_snake_case("line2Total"), "line2_total");
        assert_eq!(to_snake_case("first  name"), "first_name");
        assert_eq!(to_snake_case("already_snake"), "already_snake");
    }

    #[test]
    fn read_fields_stops_at_end_of_input() {
        let mut output = Vec::new();
        let fields = read_fields(&mut "title\nString".as_bytes(), &mut output, false, None, None, false).unwrap();
        assert_eq!(fields, vec![field("title", "String")]);
    }

    #[test]
    fn field_type_can_be_picked_from_the_menu() {
        assert_eq!(parse_field_type(""), Some("String".into()));
        assert_eq!(parse_field_type("3"), Some("i64".into()));
        assert_eq!(parse_field_type("5"), Some("bool".into()));
        assert_eq!(parse_field_type("chrono::NaiveDate"), Some("chrono::NaiveDate".into()));
        assert_eq!(parse_field_type("0"), None);
        assert_eq!(parse_field_type("42"), None);
    }

    #[test]
    fn read_fields_asks_again_for_an_unknown_type_number() {
        let mut output = Vec::new();
        let fields = read_fields(&mut "done\n42\n5\n\n".as_bytes(), &mut output, false, None, None, false).unwrap();
        assert_eq!(fields, vec![field("done", "bool")]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("1) String  2) i32"));
        assert!(output.contains("`42` is not in the list, pick 1 to 11"));
    }

    #[test]
    fn field_type_can_be_made_optional_with_a_question_mark() {
        assert_eq!(parse_field_type("?"), Some("Option<String>".into()));
        assert_eq!(parse_field_type("3?"), Some("Option<i64>".into()));
        assert_eq!(parse_field_type("NaiveDate?"), Some("Option<NaiveDate>".into()));
        assert_eq!(parse_field_type("Option<i32>?"), Some("Option<i32>".into()));
        assert_eq!(parse_field_type("42?"), None);
    }

    #[test]
    fn read_fields_accepts_inline_types_and_removes_the_last_field() {
        let input = "title:String\nstars:2?\nviews\n3\n-\nscore: f64\nbad:42\n5\n\n";
        let mut output = Vec::new();
        let fields = read_fields(&mut input.as_bytes(), &mut output, false, None, None, false).unwrap();
        assert_eq!(fields, vec![
            field("title", "String"),
            field("stars", "Option<i32>"),
            field("score", "f64"),
            field("bad", "bool"),
        ]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Field `views` removed"));
        assert!(output.contains("`42` is not in the list, pick 1 to 11"));
        assert!(output.contains("4 fields declared"));
        assert!(output.contains("stars: Option<i32> (nullable)"));
        assert!(!output.contains('\x1b'));
    }

    #[test]
    fn sqlx_flag_requires_fields() {
        assert!(Opt::from_iter_safe(&["octopux", "generate-model", "--name", "Project", "--sqlite", "--sqlx"]).is_err());
        assert!(Opt::from_iter_safe(&["octopux", "generate-model", "--name", "Project", "--sqlite", "--sqlx", "--fields"]).is_ok());
    }

    #[test]
    fn sqlx_model_derives_the_model_traits() {
        let fields = vec![
            field("title", "String"),
            field("stars", "i32"),
        ];
        let model = render_model("Project", false, false, true, false, &fields, Dialect::Sqlite);
        assert!(!super::EMPTY_BODIES.iter().any(|body| model.contains(body)));
        assert!(!model.contains("impl "));
        assert!(!model.contains("async_trait"));
        assert!(!model.contains("SELECT"));
        assert!(model.contains("        SqlxModel,\n        SqlxNewModel,\n        SqlxUpdatableModel,\n        octopux_info,\n    };"));
        assert!(model.contains("#[derive(Default, Serialize, Deserialize, sqlx::FromRow, HttpFindListDelete, SqlxModel)]\n    #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]\n    #[sqlx_model(database = \"sqlite\")]\n    #[octopux_info(path = \"project\")]"));
        assert!(model.contains("#[derive(Serialize, Deserialize, HttpCreate, SqlxNewModel)]\n    #[http_create(SaveQuery, AppState)]\n    #[sqlx_model(database = \"sqlite\", model = \"Project\")]\n    pub struct NewProject {"));
        assert!(model.contains("#[derive(Serialize, Deserialize, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]\n    #[http_update(Id, UpdateQuery, Project, FindQuery, AppState)]\n    #[sqlx_model(database = \"sqlite\")]\n    pub struct UpdatableProject {"));
        assert!(!model.contains("{model") && !model.contains("_sqlx}") && !model.contains("_impl}"));
    }

    #[test]
    fn graphql_requires_fields() {
        assert!(Opt::from_iter_safe(&["octopux", "generate-model", "--name", "Project", "--sqlite", "--graphql"]).is_err());
        assert!(Opt::from_iter_safe(&["octopux", "generate-model", "--name", "Project", "--sqlite", "--graphql", "--fields"]).is_ok());
    }

    #[test]
    fn default_model_has_no_graphql() {
        let model = render_model("Project", false, false, false, false, &[field("title", "String")], Dialect::Sqlite);
        assert!(!model.contains("async_graphql"));
        assert!(!model.contains("SimpleObject") && !model.contains("InputObject"));
        assert!(!model.contains("{graphql"));
    }

    #[test]
    fn graphql_model_derives_the_graphql_types_and_resolvers() {
        let fields = vec![field("title", "String")];
        let model = render_model("BookAuthor", true, true, false, false, &fields, Dialect::Sqlite);
        assert!(model.contains("use async_graphql::{ComplexObject, Context, InputObject, Object, SimpleObject};"));
        assert!(model.contains("#[derive(Default, Serialize, Deserialize, JsonSchema, ApiComponent, SimpleObject, HttpFindListDelete)]"));
        assert!(model.contains("#[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate)]\n    #[http_create(SaveQuery, AppState)]\n    pub struct NewBookAuthor {"));
        assert!(model.contains("#[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpUpdate)]"));
        assert!(model.contains("pub struct BookAuthorQuery;"));
        assert!(model.contains("pub struct BookAuthorMutation;"));
        assert!(model.contains("`#[derive(MergedObject, Default)] struct Query(book_author::BookAuthorQuery, ...);`"));
        assert!(model.contains("async fn book_author(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<BookAuthor> {"));
        assert!(model.contains("async fn book_authors(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<BookAuthor>> {\n            Ok(BookAuthor::list(&ListQuery {}, app_state(ctx)?).await?)"));
        assert!(model.contains("async fn create_book_author(&self, ctx: &Context<'_>, input: NewBookAuthor)"));
        assert!(model.contains("async fn update_book_author(&self, ctx: &Context<'_>, input: UpdatableBookAuthor)"));
        assert!(model.contains("async fn delete_book_author(&self, ctx: &Context<'_>, id: Id)"));
        // the traits are already imported to be implemented
        assert_eq!(model.matches("        Model,\n").count(), 1);
        assert!(!model.contains("{graphql") && !model.contains("{list_") && !model.contains("{plural}") && !model.contains("{module}"));
    }

    #[test]
    fn graphql_sqlx_model_paginates_the_list_and_imports_the_model_traits() {
        let fields = vec![field("title", "String")];
        let model = render_model("Project", false, true, true, false, &fields, Dialect::Sqlite);
        assert!(model.contains("        SqlxUpdatableModel,\n        octopux_info,\n        Model,\n        NewModel,\n        UpdatableModel,\n    };"));
        assert!(model.contains("async fn projects(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Project>> {\n            Ok(Project::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)"));
        assert!(model.contains("#[derive(Default, Serialize, Deserialize, SimpleObject, sqlx::FromRow, HttpFindListDelete, SqlxModel)]"));
    }

    #[test]
    fn sqlx_model_paginates_the_list() {
        let fields = vec![field("title", "String")];
        let model = render_model("Project", false, false, true, false, &fields, Dialect::Sqlite);
        assert!(model.contains("pub struct ListQuery {\n        /// Number of rows to skip\n        pub offset: Option<usize>,"));
        assert!(model.contains("        pub limit: Option<usize>,\n    }"));
        // without --sqlx the list query stays empty
        let model = render_model("Project", false, false, false, false, &fields, Dialect::Sqlite);
        assert!(model.contains("pub struct ListQuery {}"));
        assert!(model.contains("async fn list(_query: &ListQuery, _state: &AppState)"));
    }

    #[test]
    fn sqlx_model_sets_timestamps_and_soft_deletes() {
        let fields = vec![field("title", "String")];
        let model = render_model("Project", false, false, true, true, &fields, Dialect::Sqlite);
        assert!(model.contains("use chrono::{DateTime, Utc};"));
        assert_eq!(model.matches("#[sqlx_model(database = \"sqlite\", timestamps, soft_delete)]").count(), 2);
        assert!(model.contains("#[sqlx_model(database = \"sqlite\", model = \"Project\", timestamps)]"));
    }

    #[test]
    fn sqlx_model_targets_the_database() {
        let fields = vec![field("title", "String")];
        let postgres = render_model("Project", false, false, true, false, &fields, Dialect::Postgres);
        assert_eq!(postgres.matches("database = \"postgres\"").count(), 3);
        let mysql = render_model("Project", true, false, true, false, &fields, Dialect::Mysql);
        assert_eq!(mysql.matches("database = \"mysql\"").count(), 3);
        assert!(mysql.contains("#[derive(Serialize, Deserialize, JsonSchema, ApiComponent, HttpCreate, SqlxNewModel)]"));
    }

    #[test]
    fn migration_flag_requires_fields() {
        assert!(Opt::from_iter_safe(&["octopux", "generate-model", "--name", "Project", "--sqlite", "--migration"]).is_err());
        assert!(Opt::from_iter_safe(&["octopux", "generate-model", "--name", "Project", "--sqlite", "--migration", "--fields"]).is_ok());
    }

    #[test]
    fn migration_creates_the_model_table() {
        let fields = vec![
            field("title", "String"),
            field("stars", "i32"),
            field("views", "i64"),
            field("score", "f64"),
            field("done", "bool"),
            field("summary", "Option<String>"),
            field("cover", "Option<Vec<u8>>"),
            field("hits", "u64"),
        ];
        assert_eq!(render_migration("Project", &fields, false, Dialect::Sqlite).unwrap(), "CREATE TABLE IF NOT EXISTS project (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    stars INTEGER NOT NULL,
    views INTEGER NOT NULL,
    score REAL NOT NULL,
    done BOOLEAN NOT NULL,
    summary TEXT,
    cover BLOB,
    hits INTEGER NOT NULL
);
");
        assert_eq!(render_migration("Project", &[], false, Dialect::Sqlite).unwrap(), "CREATE TABLE IF NOT EXISTS project (\n    id INTEGER PRIMARY KEY AUTOINCREMENT\n);\n");
    }

    #[test]
    fn sources_go_in_the_src_folder_when_it_exists() {
        let root = std::env::temp_dir().join(format!("octopux-source-path-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(source_path(&root, None, "project.rs"), "project.rs");
        std::fs::create_dir_all(root.join("src")).unwrap();
        assert_eq!(source_path(&root, None, "project.rs"), "src/project.rs");
        assert_eq!(source_path(&root.join("src"), None, "project.rs"), "project.rs");
        // --output wins over the src folder
        assert_eq!(source_path(&root, Some(std::path::Path::new("src/models")), "project.rs"), "src/models/project.rs");
        assert_eq!(source_path(&root, Some(std::path::Path::new("/tmp/out")), "project.rs"), "/tmp/out/project.rs");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn migrations_go_next_to_the_src_folder() {
        use std::path::{Path, PathBuf};
        assert_eq!(migrations_dir(Path::new("/app")), PathBuf::from("migrations"));
        assert_eq!(migrations_dir(Path::new("/app/src")), PathBuf::from("../migrations"));
        assert_eq!(migrations_dir(Path::new("/app/src/models")), PathBuf::from("../../migrations"));
    }

    #[test]
    fn migration_rejects_types_without_column_type() {
        let fields = vec![
            field("tags", "Vec<String>"),
            field("title", "String"),
            field("meta", "Option<serde_json::Value>"),
        ];
        assert_eq!(
            render_migration("Project", &fields, false, Dialect::Sqlite),
            Err("no SQLite column type for tags: Vec<String>, meta: Option<serde_json::Value>".to_string())
        );
    }

    #[test]
    fn read_fields_asks_again_for_a_type_without_column_type() {
        let mut output = Vec::new();
        let input = "tags\nVec<String>\nchrono::NaiveDate\n\n";
        let fields = read_fields(&mut input.as_bytes(), &mut output, false, Some(Dialect::Sqlite), None, false).unwrap();
        assert_eq!(fields, vec![field("tags", "chrono::NaiveDate")]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("`Vec<String>` has no SQLite column type, use one of String, i8"));
        // without a migration any type is accepted
        let fields = read_fields(&mut "tags\nVec<String>\n\n".as_bytes(), &mut Vec::new(), false, None, None, false).unwrap();
        assert_eq!(fields, vec![field("tags", "Vec<String>")]);
    }

    #[test]
    fn timestamps_are_added_to_the_model_and_updatable_structs() {
        let fields = vec![field("title", "String")];
        let model = render_model("Project", false, false, false, true, &fields, Dialect::Sqlite);
        assert!(model.contains("use chrono::{DateTime, Utc};"));
        assert!(model.contains("pub struct Project {\n        pub id: Id,\n        pub title: String,\n        pub created_at: Option<DateTime<Utc>>,\n        pub updated_at: Option<DateTime<Utc>>,\n        pub deleted_at: Option<DateTime<Utc>>,\n    }"));
        assert!(model.contains("pub struct NewProject {\n        pub title: String,\n    }"));
        assert!(model.contains("pub struct UpdatableProject {\n        pub id: Id,\n        pub title: String,\n        pub updated_at: Option<DateTime<Utc>>,\n    }"));
        assert!(!render_model("Project", false, false, false, false, &fields, Dialect::Sqlite).contains("chrono"));
    }

    #[test]
    fn migration_adds_timestamp_columns() {
        let fields = vec![field("title", "String")];
        assert_eq!(
            render_migration("Project", &fields, true, Dialect::Sqlite).unwrap(),
            "CREATE TABLE IF NOT EXISTS project (\n    id INTEGER PRIMARY KEY AUTOINCREMENT,\n    title TEXT NOT NULL,\n    created_at DATETIME,\n    updated_at DATETIME,\n    deleted_at DATETIME\n);\n"
        );
    }

    #[test]
    fn timestamps_reserve_their_field_names() {
        let mut output = Vec::new();
        let fields = read_fields(&mut "created_at\nupdated_at\ndeleted_at\ntitle\n\n".as_bytes(), &mut output, true, None, None, false).unwrap();
        assert_eq!(fields, vec![field("title", "String")]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Field `created_at` is already declared"));
        assert!(output.contains("Field `updated_at` is already declared"));
        assert!(output.contains("Field `deleted_at` is already declared"));
    }

    #[test]
    fn migration_timestamp_is_utc_date_and_time() {
        assert_eq!(migration_timestamp(0), "19700101000000");
        assert_eq!(migration_timestamp(951782400), "20000229000000");
        assert_eq!(migration_timestamp(1790595045), "20260928113045");
    }

    #[test]
    fn date_fields_import_chrono_and_get_date_columns() {
        let fields = vec![
            field("published_at", "DateTime<Utc>"),
            field("release", "NaiveDate"),
            field("seen_at", "Option<NaiveDateTime>"),
            field("opens", "NaiveTime"),
        ];
        let model = render_model("Project", false, false, false, false, &fields, Dialect::Sqlite);
        assert!(model.contains("use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};"));
        let model = render_model("Project", false, false, false, true, &fields[1..2], Dialect::Sqlite);
        assert!(model.contains("use chrono::{DateTime, NaiveDate, Utc};"));
        // NaiveDateTime does not import DateTime
        let model = render_model("Project", false, false, false, false, &fields[2..3], Dialect::Sqlite);
        assert!(model.contains("use chrono::{NaiveDateTime};"));
        assert!(render_migration("Project", &fields, false, Dialect::Sqlite).unwrap().contains(
            "    published_at DATETIME NOT NULL,\n    release DATE NOT NULL,\n    seen_at DATETIME,\n    opens TIME NOT NULL\n"
        ));
    }

    #[test]
    fn database_flags_conflict() {
        let parse = |flags: &[&str]| {
            let mut args = vec!["octopux", "generate-model", "--name", "Project"];
            args.extend(flags);
            Opt::from_iter_safe(&args)
        };
        assert!(parse(&[]).is_err());
        assert!(parse(&["--sqlite"]).is_ok());
        assert!(parse(&["--postgres"]).is_ok());
        assert!(parse(&["--postgres", "--mysql"]).is_err());
        assert!(parse(&["--sqlite", "--postgres"]).is_err());
        assert!(parse(&["--sqlite", "--mysql"]).is_err());
        assert_eq!(Dialect::from_flags(false, false), Dialect::Sqlite);
        assert_eq!(Dialect::from_flags(true, false), Dialect::Postgres);
        assert_eq!(Dialect::from_flags(false, true), Dialect::Mysql);
    }

    #[test]
    fn postgres_migration_uses_postgres_types() {
        let fields = vec![
            field("title", "String"),
            field("stars", "i32"),
            field("views", "i64"),
            field("score", "f64"),
            field("done", "bool"),
            field("tags", "Vec<String>"),
            field("cover", "Option<Vec<u8>>"),
            field("at", "DateTime<Utc>"),
            field("ratios", "Vec<f32>"),
            field("seen", "Vec<DateTime<Utc>>"),
            field("days", "Option<Vec<NaiveDate>>"),
            field("logs", "Vec<NaiveDateTime>"),
            field("slots", "Vec<NaiveTime>"),
            field("blobs", "Vec<Vec<u8>>"),
        ];
        assert_eq!(render_migration("Project", &fields, true, Dialect::Postgres).unwrap(), "CREATE TABLE IF NOT EXISTS project (
    id BIGSERIAL PRIMARY KEY,
    title VARCHAR(255) NOT NULL,
    stars INT4 NOT NULL,
    views INT8 NOT NULL,
    score FLOAT8 NOT NULL,
    done BOOL NOT NULL,
    tags TEXT[] NOT NULL,
    cover BYTEA,
    at TIMESTAMPTZ NOT NULL,
    ratios FLOAT4[] NOT NULL,
    seen TIMESTAMPTZ[] NOT NULL,
    days DATE[],
    logs TIMESTAMP[] NOT NULL,
    slots TIME[] NOT NULL,
    blobs BYTEA[] NOT NULL,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);
");
        let unsigned = vec![field("count", "u32")];
        assert_eq!(
            render_migration("Project", &unsigned, false, Dialect::Postgres),
            Err("no PostgreSQL column type for count: u32".to_string())
        );
    }

    #[test]
    fn mysql_migration_uses_mysql_types() {
        let fields = vec![
            field("title", "String"),
            field("stars", "i32"),
            field("count", "u32"),
            field("score", "f64"),
            field("done", "bool"),
            field("summary", "Option<String>"),
        ];
        assert_eq!(render_migration("Project", &fields, true, Dialect::Mysql).unwrap(), "CREATE TABLE IF NOT EXISTS project (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    title VARCHAR(255) NOT NULL,
    stars INT NOT NULL,
    count INT UNSIGNED NOT NULL,
    score DOUBLE NOT NULL,
    done BOOLEAN NOT NULL,
    summary VARCHAR(255),
    created_at DATETIME(6),
    updated_at DATETIME(6),
    deleted_at DATETIME(6)
);
");
        let tags = vec![field("tags", "Vec<String>")];
        assert!(render_migration("Project", &tags, false, Dialect::Mysql).is_err());
    }

    #[test]
    fn read_fields_checks_types_against_the_database() {
        let mut output = Vec::new();
        let fields = read_fields(&mut "tags\nVec<String>\n\n".as_bytes(), &mut output, false, Some(Dialect::Postgres), None, false).unwrap();
        assert_eq!(fields, vec![field("tags", "Vec<String>")]);
        let mut output = Vec::new();
        let fields = read_fields(&mut "count\nu32\ni64\n\n".as_bytes(), &mut output, false, Some(Dialect::Postgres), None, false).unwrap();
        assert_eq!(fields, vec![field("count", "i64")]);
        assert!(String::from_utf8(output).unwrap().contains("`u32` has no PostgreSQL column type, use one of String, i16"));
    }

    fn relation(parent: &str, child: &str, through: Option<&str>) -> Relation {
        Relation::new(parent.into(), child.into(), None, None, through.map(String::from), None)
    }

    #[test]
    fn relation_names_default_to_the_plural_of_the_child() {
        assert_eq!(pluralize("book"), "books");
        assert_eq!(pluralize("category"), "categories");
        assert_eq!(pluralize("day"), "days");
        assert_eq!(pluralize("box"), "boxes");
        assert_eq!(pluralize("branch"), "branches");
        assert_eq!(to_camel_case("favorite_books"), "FavoriteBooks");
        let books = relation("Project", "Book", None);
        assert_eq!(books.name, "books");
        assert_eq!(books.foreign_key, "project_id");
        assert_eq!(books.module(), "project_books");
        assert_eq!(books.type_name(), "ProjectBooks");
        let categories = relation("Project", "ProjectCategory", None);
        assert_eq!(categories.name, "project_categories");
        assert_eq!(categories.type_name(), "ProjectProjectCategories");
        let through = relation("Project", "Category", Some("ProjectCategory"));
        assert_eq!(through.through, Some(Through { model: "ProjectCategory".into(), table: "project_category".into(), child_key: "category_id".into() }));
        let named = Relation::new("Project".into(), "Book".into(), Some("drafts".into()), Some("owner_id".into()), None, None);
        assert_eq!((named.name.as_str(), named.foreign_key.as_str(), named.module()), ("drafts", "owner_id", "project_drafts".to_string()));
    }

    #[test]
    fn relation_implements_has_many_on_its_own_type() {
        let rel = render_relation(&relation("Project", "Book", None), false, false, false, false, Dialect::Sqlite);
        assert!(rel.contains("use super::project::{Project, Id};\n    use super::book::Book;"));
        assert!(rel.contains("pub struct ProjectBooksQuery {\n        /// Number of rows to skip\n        pub offset: Option<usize>,"));
        assert!(rel.contains("pub struct ProjectBooks;"));
        assert!(rel.contains("impl HasMany for ProjectBooks {\n        type Parent = Project;\n        type Id = Id;\n        type Query = ProjectBooksQuery;\n        type Result = Vec<Book>;\n        type State = AppState;\n        const RELATION: &'static str = \"books\";"));
        assert!(rel.contains("async fn list_related(id: Id, _query: &ProjectBooksQuery, _state: &AppState) -> Result<Option<Vec<Book>>> {"));
        assert!(rel.contains("pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {\n        gen_relation_endpoint!(ProjectBooks)(cfg)\n    }"));
        assert!(rel.contains("`.configure(project_books::configure)`"));
        assert!(!rel.contains("JsonSchema"));
        assert!(!rel.contains("sqlx"));
    }

    #[test]
    fn self_referencing_relation_imports_its_model_once() {
        let rel = render_relation(&relation("Category", "Category", None), false, false, false, false, Dialect::Sqlite);
        assert!(rel.contains("use super::category::{Category, Id};\n    use serde::Deserialize;"));
        assert_eq!(rel.matches("use super::category::").count(), 1);
    }

    #[test]
    fn openapi_relation_is_documented() {
        let rel = render_relation(&relation("Project", "Book", None), true, false, false, false, Dialect::Sqlite);
        assert!(rel.contains("#[derive(Deserialize, JsonSchema, ApiComponent)]\n    pub struct ProjectBooksQuery"));
        assert!(rel.contains("pub fn configure(cfg: &mut apistos::web::ServiceConfig) {\n        gen_documented_relation_endpoint!(ProjectBooks)(cfg)\n    }"));
    }

    #[test]
    fn sqlx_relation_pages_the_children_and_looks_up_the_parent() {
        let rel = render_relation(&relation("Project", "Book", None), false, false, true, false, Dialect::Sqlite);
        assert!(rel.contains("const DEFAULT_LIMIT: i64 = 20;"));
        assert!(rel.contains("async fn list_related(id: Id, query: &ProjectBooksQuery, state: &AppState)"));
        assert!(rel.contains("let models = sqlx::query_as::<_, Book>(\n                \"SELECT * FROM book WHERE project_id = $1 ORDER BY id LIMIT $2 OFFSET $3\",\n            )\n            .bind(id)\n            .bind(limit)\n            .bind(offset)\n            .fetch_all(&state.pool)"));
        assert!(rel.contains("if models.is_empty() {\n                let parent = sqlx::query_scalar::<_, Id>(\n                    \"SELECT id FROM project WHERE id = $1\","));
        assert!(rel.contains("if parent.is_none() {\n                    return Ok(None);\n                }\n            }\n            Ok(Some(models))"));
        let mysql = render_relation(&relation("Project", "Book", None), false, false, true, true, Dialect::Mysql);
        assert!(mysql.contains("\"SELECT * FROM book WHERE project_id = ? AND deleted_at IS NULL ORDER BY id LIMIT ? OFFSET ?\""));
        assert!(mysql.contains("\"SELECT id FROM project WHERE id = ? AND deleted_at IS NULL\""));
    }

    #[test]
    fn sqlx_relation_joins_the_through_table() {
        let rel = render_relation(&relation("Project", "Category", Some("ProjectCategory")), false, false, true, false, Dialect::Postgres);
        assert!(rel.contains("\"SELECT category.* FROM category JOIN project_category ON project_category.category_id = category.id WHERE project_category.project_id = $1 ORDER BY category.id LIMIT $2 OFFSET $3\""));
        let rel = render_relation(&relation("Project", "Category", Some("ProjectCategory")), false, false, true, true, Dialect::Postgres);
        assert!(rel.contains("WHERE project_category.project_id = $1 AND category.deleted_at IS NULL AND project_category.deleted_at IS NULL ORDER BY"));
    }

    #[test]
    fn relation_migration_indexes_the_foreign_key() {
        assert_eq!(
            render_relation_migration(&relation("Project", "Book", None), Dialect::Sqlite),
            "CREATE INDEX IF NOT EXISTS book_project_id_idx ON book (project_id);\n"
        );
        assert_eq!(
            render_relation_migration(&relation("Project", "Category", Some("ProjectCategory")), Dialect::Mysql),
            "CREATE INDEX project_category_project_id_idx ON project_category (project_id);\n"
        );
    }

    #[test]
    fn child_key_requires_through() {
        let parse = |flags: &[&str]| {
            let mut args = vec!["octopux", "generate-relation", "--parent", "Project", "--child", "Category", "--sqlite"];
            args.extend(flags);
            Opt::from_iter_safe(&args)
        };
        assert!(parse(&[]).is_ok());
        assert!(parse(&["--child-key", "cat_id"]).is_err());
        assert!(parse(&["--through", "ProjectCategory", "--child-key", "cat_id"]).is_ok());
        assert!(parse(&["--postgres", "--mysql"]).is_err());
    }

    #[test]
    fn force_is_accepted_by_the_generators() {
        assert!(Opt::from_iter_safe(&["octopux", "generate-model", "--name", "Project", "--sqlite", "--force"]).is_ok());
        assert!(Opt::from_iter_safe(&["octopux", "generate-relation", "--parent", "Project", "--child", "Book", "--sqlite", "--force"]).is_ok());
        let opt = Opt::from_iter_safe(&["octopux", "generate-model", "--name", "Project", "--sqlite", "--output=src/models"]).unwrap();
        assert!(matches!(opt, Opt::GenerateModel { output: Some(ref o), .. } if o == std::path::Path::new("src/models")));
        let opt = Opt::from_iter_safe(&["octopux", "generate-relation", "--parent", "Project", "--child", "Book", "--sqlite", "--output", "/tmp/out"]).unwrap();
        assert!(matches!(opt, Opt::GenerateRelation { output: Some(ref o), .. } if o == std::path::Path::new("/tmp/out")));
    }

    #[test]
    fn bootstrap_is_a_root_flag() {
        let cli = Cli::from_iter_safe(&["octopux", "--bootstrap", "--sqlite"]).unwrap();
        assert!(cli.bootstrap && cli.cmd.is_none());
        let cli = Cli::from_iter_safe(&["octopux", "generate-model", "--name", "Project", "--sqlite"]).unwrap();
        assert!(!cli.bootstrap && cli.cmd.is_some());
        assert!(Cli::from_iter_safe(&["octopux", "--bootstrap", "--sqlite", "--openapi"]).unwrap().openapi);
        assert!(Cli::from_iter_safe(&["octopux", "--openapi"]).is_err());
    }

    #[test]
    fn bootstrap_writes_main_and_helpers() {
        let root = std::env::temp_dir().join(format!("octopux-bootstrap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        bootstrap(&root, false, false, Dialect::Sqlite, &mut "".as_bytes(), &mut Vec::new()).unwrap();
        let main = std::fs::read_to_string(root.join("src/main.rs")).unwrap();
        let helpers = std::fs::read_to_string(root.join("src/helpers.rs")).unwrap();
        assert!(main.starts_with(&(generated_header("//") + "mod helpers;\nuse ")));
        assert!(!main.contains("mod project;"));
        assert!(main.contains(" web::scope(\"v1\"),"));
        assert!(!main.contains("apistos"));
        assert!(!main.contains(".configure(project::configure)"));
        assert!(helpers.contains("pub pool: SqlitePool,"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn openapi_bootstrap_writes_an_apistos_main() {
        let root = std::env::temp_dir().join(format!("octopux-bootstrap-openapi-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        bootstrap(&root, true, false, Dialect::Sqlite, &mut "".as_bytes(), &mut Vec::new()).unwrap();
        let main = std::fs::read_to_string(root.join("src/main.rs")).unwrap();
        let helpers = std::fs::read_to_string(root.join("src/helpers.rs")).unwrap();
        assert!(main.starts_with(&(generated_header("//") + "mod helpers;\nuse ")));
        assert!(!main.contains("mod project;"));
        assert!(main.contains("apistos::web::scope(\"v1\"),"));
        assert!(!main.contains(".configure(project::configure)"));
        assert!(helpers.contains("pub pool: SqlitePool,"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn bootstrap_overwrites_existing_main_once_confirmed() {
        let root = std::env::temp_dir().join(format!("octopux-bootstrap-overwrite-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
        let mut output = Vec::new();
        bootstrap(&root, false, false, Dialect::Sqlite, &mut "y\n".as_bytes(), &mut output).unwrap();
        assert!(String::from_utf8(output).unwrap().contains("main.rs already exists, overwrite it with the generated one? (y/N) "));
        assert!(std::fs::read_to_string(root.join("src/main.rs")).unwrap().starts_with(&(generated_header("//") + "mod helpers;\n")));
        assert!(root.join("src/helpers.rs").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn graphql_bootstrap_writes_a_main_serving_the_schema() {
        let root = std::env::temp_dir().join(format!("octopux-bootstrap-graphql-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        bootstrap(&root, false, true, Dialect::Sqlite, &mut "".as_bytes(), &mut Vec::new()).unwrap();
        let main = std::fs::read_to_string(root.join("src/main.rs")).unwrap();
        assert_eq!(main, super::with_header("//", &render_bootstrap_main(false, true, Dialect::Sqlite)));
        assert!(main.contains("struct Query(ApiQuery);") && main.contains("type Mutation = EmptyMutation;"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn graphql_roots_are_merged_into_the_bootstrapped_main_on_disk() {
        let root = std::env::temp_dir().join(format!("octopux-graphql-roots-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        bootstrap(&root, false, true, Dialect::Sqlite, &mut "".as_bytes(), &mut Vec::new()).unwrap();
        super::add_graphql_roots(&root, "author", "Author").unwrap();
        super::add_graphql_roots(&root, "book", "Book").unwrap();
        let main = std::fs::read_to_string(root.join("src/main.rs")).unwrap();
        assert!(main.contains("struct Query(ApiQuery, author::AuthorQuery, book::BookQuery);"));
        assert!(main.contains("#[derive(MergedObject, Default)]\nstruct Mutation(author::AuthorMutation, book::BookMutation);"));
        assert!(!main.contains("EmptyMutation"));
        // merging again leaves the file as it is
        super::add_graphql_roots(&root, "author", "Author").unwrap();
        assert_eq!(std::fs::read_to_string(root.join("src/main.rs")).unwrap(), main);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn graphql_roots_are_not_written_without_a_graphql_main() {
        let root = std::env::temp_dir().join(format!("octopux-graphql-roots-plain-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        // no src/main.rs at all
        std::fs::create_dir_all(root.join("src")).unwrap();
        super::add_graphql_roots(&root, "author", "Author").unwrap();
        assert!(!root.join("src/main.rs").exists());
        // a main bootstrapped without --graphql
        std::fs::remove_dir_all(&root).unwrap();
        bootstrap(&root, false, false, Dialect::Sqlite, &mut "".as_bytes(), &mut Vec::new()).unwrap();
        let main = std::fs::read_to_string(root.join("src/main.rs")).unwrap();
        super::add_graphql_roots(&root, "author", "Author").unwrap();
        assert_eq!(std::fs::read_to_string(root.join("src/main.rs")).unwrap(), main);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn relation_field_is_added_to_the_parent_model_on_disk() {
        let root = std::env::temp_dir().join(format!("octopux-relation-field-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        let parent = super::with_header("//", &render_model("Author", false, true, true, false, &[field("name", "String")], Dialect::Sqlite));
        std::fs::write(root.join("src/author.rs"), &parent).unwrap();
        let rel = relation("Author", "Book", None);
        super::add_relation_field(&root, None, &rel).unwrap();
        let patched = std::fs::read_to_string(root.join("src/author.rs")).unwrap();
        assert_eq!(patched, super::with_relation_field(&parent, &rel).unwrap());
        super::add_relation_field(&root, None, &rel).unwrap();
        assert_eq!(std::fs::read_to_string(root.join("src/author.rs")).unwrap(), patched);

        // a parent generated without --graphql is left as it is, the field being printed to add by hand
        let plain = super::with_header("//", &render_model("Author", false, false, true, false, &[field("name", "String")], Dialect::Sqlite));
        std::fs::write(root.join("src/author.rs"), &plain).unwrap();
        super::add_relation_field(&root, None, &rel).unwrap();
        assert_eq!(std::fs::read_to_string(root.join("src/author.rs")).unwrap(), plain);
        // as is a missing parent
        std::fs::remove_file(root.join("src/author.rs")).unwrap();
        super::add_relation_field(&root, None, &rel).unwrap();
        assert!(!root.join("src/author.rs").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn generate_relation_accepts_graphql() {
        let opt = Opt::from_iter_safe(&["octopux", "generate-relation", "--parent", "Author", "--child", "Book", "--sqlite", "--graphql"]).unwrap();
        assert!(matches!(opt, Opt::GenerateRelation { graphql: true, .. }));
    }

    #[test]
    fn generated_header_comments_the_logo() {
        let header = generated_header("--");
        assert!(header.trim_end().lines().all(|line| line.starts_with("--")));
        assert!(header.ends_with("--------\n\n"));
        assert_eq!(header.lines().count(), LOGO.trim_matches('\n').lines().count() + 1);
    }

    #[test]
    fn confirm_defaults_to_no() {
        let answer = |input: &str| confirm(&mut input.as_bytes(), &mut Vec::new(), "Install?").unwrap();
        assert!(answer("y\n"));
        assert!(answer("YES\n"));
        assert!(!answer("\n"));
        assert!(!answer("n\n"));
        assert!(!answer("maybe\n"));
        assert!(!answer(""));
    }

    #[test]
    fn confirm_save_defaults_to_yes() {
        let files = [("project.rs".to_string(), false)];
        let answer = |input: &str| confirm_save(&mut input.as_bytes(), &mut Vec::new(), &files).unwrap();
        assert!(answer("\n"));
        assert!(answer("y\n"));
        assert!(answer(""));
        assert!(!answer("n\n"));
        assert!(!answer("NO\n"));
    }

    #[test]
    fn changes_summary_lists_the_created_and_overwritten_files() {
        let summary = changes_summary(&[("project.rs".to_string(), true), ("migrations/1_create_project.sql".to_string(), false)]);
        assert!(summary.starts_with("2 files to write"));
        assert!(summary.contains("  ~ project.rs (overwritten)"));
        assert!(summary.contains("  + migrations/1_create_project.sql"));
    }

    #[test]
    fn graphql_bootstrap_requires_bootstrap() {
        assert!(Cli::from_iter_safe(&["octopux", "--bootstrap", "--sqlite", "--graphql"]).unwrap().graphql);
        assert!(Cli::from_iter_safe(&["octopux", "--graphql"]).is_err());
    }

    #[test]
    fn graphql_bootstrap_serves_the_schema_and_graphiql() {
        for openapi in [false, true] {
            let main = render_bootstrap_main(openapi, true, Dialect::Sqlite);
            assert!(main.contains("use async_graphql_actix_web::GraphQL;"));
            assert!(main.contains("#[derive(MergedObject, Default)]\nstruct Query(ApiQuery);"));
            assert!(main.contains("type Mutation = EmptyMutation;"));
            assert!(main.contains("let schema = Schema::build(Query::default(), Mutation::default(), EmptySubscription)\n        .data(state.clone())"));
            assert!(main.contains(".route(web::post().to(GraphQL::new(schema.clone())))\n                    .route(web::get().to(graphiql)),"));
            assert!(!main.contains("{graphql"));
            assert!(main.starts_with("#![recursion_limit = \"512\"]\n\nmod helpers;"));
        }
        // the plain actix app has no route after apistos builds the app
        assert!(render_bootstrap_main(true, true, Dialect::Sqlite).contains("SwaggerUIConfig::new(&\"/swagger\")),\n            )\n            // GraphQL"));
        for openapi in [false, true] {
            let main = render_bootstrap_main(openapi, false, Dialect::Sqlite);
            assert!(!main.contains("graphql") && !main.contains("{graphql"));
        }
    }

    #[test]
    fn graphql_bootstrap_adds_async_graphql() {
        let deps = bootstrap_dependencies(false, true, Dialect::Sqlite);
        assert!(deps.iter().any(|d| d == &["async-graphql@7", "--features", "chrono"]));
        assert!(deps.iter().any(|d| d == &["async-graphql-actix-web@7"]));
        assert!(!bootstrap_dependencies(true, false, Dialect::Sqlite).iter().any(|d| d[0].starts_with("async-graphql")));
    }

    #[test]
    fn graphql_model_declares_a_complex_object_for_the_relations() {
        let model = render_model("Project", false, true, true, false, &[field("name", "String")], Dialect::Sqlite);
        assert!(model.contains("#[octopux_info(path = \"project\")]\n    #[graphql(complex)]\n    pub struct Project {"));
        assert!(model.contains(&format!("#[ComplexObject]\n    impl Project {{\n{}\n    }}", super::GRAPHQL_RELATIONS_MARKER)));
        assert!(!render_model("Project", false, false, true, false, &[field("name", "String")], Dialect::Sqlite).contains("complex"));
    }

    #[test]
    fn graphql_relation_resolves_the_children_of_the_parent() {
        let rel = render_relation(&relation("Project", "Book", None), false, true, true, false, Dialect::Sqlite);
        assert!(rel.contains("pub async fn resolve(ctx: &async_graphql::Context<'_>, id: Id, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Book>> {"));
        assert!(rel.contains("ProjectBooks::list_related(id, &ProjectBooksQuery { offset, limit }, state.get_ref()).await?;"));
        assert!(!render_relation(&relation("Project", "Book", None), false, false, true, false, Dialect::Sqlite).contains("async_graphql"));
    }

    #[test]
    fn relation_field_is_inserted_once_in_the_complex_object_of_the_parent() {
        let parent = render_model("Project", false, true, true, false, &[field("name", "String")], Dialect::Sqlite);
        let rel = relation("Project", "Book", None);
        let patched = super::with_relation_field(&parent, &rel).unwrap();
        assert!(patched.contains(&format!(
            "{}\n        /// The books of the project, paginated\n        async fn books(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::book::Book>> {{\n            super::project_books::resolve(ctx, self.id, offset, limit).await\n        }}\n    }}",
            super::GRAPHQL_RELATIONS_MARKER
        )));
        assert_eq!(super::with_relation_field(&patched, &rel).unwrap(), patched);
        // a second relation goes next to the first one
        let categories = super::with_relation_field(&patched, &relation("Project", "Category", Some("ProjectCategory"))).unwrap();
        assert!(categories.contains("super::project_books::resolve(") && categories.contains("super::project_categories::resolve("));
        // a model generated without --graphql has no complex object
        let plain = render_model("Project", false, false, true, false, &[field("name", "String")], Dialect::Sqlite);
        assert!(super::with_relation_field(&plain, &rel).is_none());
    }

    #[test]
    fn graphql_roots_of_models_are_merged_once_into_the_schema() {
        let main = render_bootstrap_main(true, true, Dialect::Sqlite);
        let first = super::with_graphql_roots(&main, "project", "Project").unwrap();
        assert!(first.contains("struct Query(ApiQuery, project::ProjectQuery);"));
        assert!(first.contains("#[derive(MergedObject, Default)]\nstruct Mutation(project::ProjectMutation);"));
        assert!(!first.contains("EmptyMutation"));
        assert_eq!(super::with_graphql_roots(&first, "project", "Project").unwrap(), first);
        // the roots of a second model are appended to those of the first
        let second = super::with_graphql_roots(&first, "book_author", "BookAuthor").unwrap();
        assert!(second.contains("struct Query(ApiQuery, project::ProjectQuery, book_author::BookAuthorQuery);"));
        assert!(second.contains("struct Mutation(project::ProjectMutation, book_author::BookAuthorMutation);"));
        // a model whose name ends another one is merged too
        let author = super::with_graphql_roots(&second, "author", "Author").unwrap();
        assert!(author.contains("book_author::BookAuthorQuery, author::AuthorQuery);"));
        // a main bootstrapped without --graphql has no schema
        assert!(super::with_graphql_roots(&render_bootstrap_main(true, false, Dialect::Sqlite), "project", "Project").is_none());
    }

    #[test]
    fn bootstrap_connects_to_the_database_of_every_combination() {
        for dialect in [Dialect::Sqlite, Dialect::Postgres, Dialect::Mysql] {
            let pool = super::bootstrap_pool(dialect);
            for openapi in [false, true] {
                for graphql in [false, true] {
                    let main = render_bootstrap_main(openapi, graphql, dialect);
                    assert!(!main.contains("{pool") && !main.contains("{graphql"));
                    assert!(main.contains(&format!("use sqlx::{};", pool)));
                    assert!(main.contains(&format!("let pool = {}::connect(", pool)));
                    assert_eq!(main.contains("DATABASE_URL"), dialect != Dialect::Sqlite);
                    assert_eq!(main.contains("apistos::web::scope(\"v1\")"), openapi);
                    assert_eq!(main.contains("Schema::build("), graphql);
                }
            }
            assert!(super::render_bootstrap_helpers(dialect).contains(&format!("use sqlx::{};\n\npub struct AppState {{\n    pub pool: {},", pool, pool)));
        }
        let sqlx = |dialect| bootstrap_dependencies(false, false, dialect).into_iter().find(|d| d[0] == "sqlx@0.9").unwrap();
        assert!(sqlx(Dialect::Sqlite)[3].contains(",sqlite,"));
        assert!(sqlx(Dialect::Postgres)[3].contains(",postgres,"));
        assert!(sqlx(Dialect::Mysql)[3].contains(",mysql,"));
    }

    #[test]
    fn bootstrap_asks_the_options_without_a_database_flag() {
        let ask = |args: &[&str], answers: &str| {
            let cli = Cli::from_iter_safe(args).unwrap();
            bootstrap_options(&cli, &mut answers.as_bytes(), &mut Vec::new()).unwrap()
        };
        let options = |dialect, openapi, graphql| BootstrapOptions { dialect, openapi, graphql };
        assert_eq!(ask(&["octopux", "--bootstrap"], "2\ny\nn\n"), options(Dialect::Postgres, true, false));
        // invalid answers are asked again, names and flags are accepted
        assert_eq!(ask(&["octopux", "--bootstrap"], "9\nmysql\n\ny\n"), options(Dialect::Mysql, false, true));
        assert_eq!(ask(&["octopux", "--bootstrap"], "--sqlite\n"), options(Dialect::Sqlite, false, false));
        // the end of the input takes the defaults
        assert_eq!(ask(&["octopux", "--bootstrap"], ""), options(Dialect::Sqlite, false, false));
        // a given flag is not asked: only the GraphQL answer remains
        assert_eq!(ask(&["octopux", "--bootstrap", "--openapi"], "3\ny\n"), options(Dialect::Mysql, true, true));
        // a database flag skips every question
        assert_eq!(ask(&["octopux", "--bootstrap", "--postgres", "--graphql"], "y\ny\n"), options(Dialect::Postgres, false, true));
    }

    #[test]
    fn bootstrap_database_flags_require_bootstrap_and_conflict() {
        let cli = Cli::from_iter_safe(&["octopux", "--bootstrap", "--postgres"]).unwrap();
        assert_eq!(Dialect::from_flags(cli.postgres, cli.mysql), Dialect::Postgres);
        assert!(Cli::from_iter_safe(&["octopux", "--bootstrap", "--mysql"]).unwrap().mysql);
        assert!(Cli::from_iter_safe(&["octopux", "--bootstrap", "--sqlite"]).unwrap().sqlite);
        assert!(Cli::from_iter_safe(&["octopux", "--postgres"]).is_err());
        assert!(Cli::from_iter_safe(&["octopux", "--bootstrap"]).is_ok());
        assert!(Opt::from_iter_safe(&["octopux", "generate-relation", "--parent", "A", "--child", "B"]).is_err());
        assert!(Cli::from_iter_safe(&["octopux", "--bootstrap", "--postgres", "--mysql"]).is_err());
        assert!(Cli::from_iter_safe(&["octopux", "--bootstrap", "--sqlite", "--postgres"]).is_err());
    }

    #[test]
    fn bootstrap_dependencies_pin_octopux_to_the_cli_version() {
        let tag = format!("v{}", env!("CARGO_PKG_VERSION"));
        let deps = bootstrap_dependencies(true, false, Dialect::Sqlite);
        assert_eq!(deps[0], ["octopux", "--git", "https://github.com/ctaque/octopux", "--tag", tag.as_str(), "--features", "openapi,sqlx"]);
        assert!(deps.iter().any(|d| d == &["apistos-schemars@0.8", "--rename", "schemars"]));
        let deps = bootstrap_dependencies(false, false, Dialect::Sqlite);
        assert_eq!(deps[0], ["octopux", "--git", "https://github.com/ctaque/octopux", "--tag", tag.as_str(), "--features", "sqlx"]);
        assert!(!deps.iter().any(|d| d[0].starts_with("apistos")));
    }

    #[test]
    fn table_option_names_the_table_of_the_sqlx_derives() {
        let fields = [field("title", "String")];
        let model = render_table_model("BookPage", Some("book_page"), false, false, true, false, &fields, Dialect::Sqlite);
        assert!(model.contains("#[sqlx_model(database = \"sqlite\", table = \"book_page\")]\n    #[octopux_info(path = \"bookpage\")]\n    pub struct BookPage {"));
        assert!(model.contains("#[sqlx_model(database = \"sqlite\", model = \"BookPage\", table = \"book_page\")]\n    pub struct NewBookPage {"));
        assert!(model.contains("#[sqlx_model(database = \"sqlite\", table = \"book_page\")]\n    pub struct UpdatableBookPage {"));
        // the default table is left to the derives
        assert_eq!(render_table_model("BookPage", Some("bookpage"), false, false, true, false, &fields, Dialect::Sqlite), render_model("BookPage", false, false, true, false, &fields, Dialect::Sqlite));
        let opt = Opt::from_iter_safe(["octopux", "generate-model", "--name", "BookPage", "--sqlite", "--table", "book_page"]).unwrap();
        assert!(matches!(opt, Opt::GenerateModel { table: Some(ref t), .. } if t == "book_page"));
        // the GraphQL fields keep the model name, the root to merge names the module of the table
        let model = render_table_model("BookPage", Some("pages"), false, true, true, false, &fields, Dialect::Sqlite);
        assert!(model.contains("async fn book_page(") && model.contains("async fn create_book_page(") && model.contains("struct Query(pages::BookPageQuery, ...)"));
    }

    #[test]
    fn keyword_fields_are_raw_identifiers() {
        let model = render_model("Project", false, false, false, false, &[field("type", "i32"), field("name", "String")], Dialect::Sqlite);
        assert!(model.contains("pub r#type: i32,\n        pub name: String,"));
        assert!(super::is_field_name("type"));
        assert!(!super::is_field_name("self"));
    }

    #[test]
    fn relation_tables_replace_the_lowercase_models() {
        let relation = Relation::new("Project".into(), "Category".into(), None, None, Some("ProjectCategory".into()), None).with_tables(
            Some("projects".into()),
            Some("categories".into()),
            Some("project_category".into()),
        );
        let body = render_relation(&relation, false, false, true, false, Dialect::Sqlite);
        assert!(body.contains("SELECT categories.* FROM categories JOIN project_category ON project_category.category_id = categories.id WHERE project_category.project_id = $1"));
        assert!(body.contains("SELECT id FROM projects WHERE id = $1"));
        // the modules are named after the tables
        assert!(body.contains("use super::projects::{Project, Id};\n    use super::categories::Category;"));
        assert_eq!(relation.module(), "projects_categories");
        assert_eq!(render_relation_migration(&relation, Dialect::Sqlite), "CREATE INDEX IF NOT EXISTS project_category_project_id_idx ON project_category (project_id);\n");
        assert!(Opt::from_iter_safe(["octopux", "generate-relation", "--parent", "A", "--child", "B", "--sqlite", "--through-table", "ab"]).is_err());
    }

    #[test]
    fn migration_tables_replay_the_altered_columns_and_the_unique_indexes() {
        let dir = std::env::temp_dir().join(format!("octopux-migration-alters-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("1_create_book.sql"), "-- header; with a semicolon\nCREATE TABLE IF NOT EXISTS book (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, old TEXT);\nCREATE TABLE tmp (id INTEGER);").unwrap();
        std::fs::write(dir.join("2_alter_book.sql"), "ALTER TABLE book ADD COLUMN isbn TEXT DEFAULT 'a;b';
ALTER TABLE IF EXISTS book ADD COLUMN IF NOT EXISTS author_id INT8 NOT NULL REFERENCES author (id), DROP COLUMN old;
alter table book rename column title to name;
CREATE UNIQUE INDEX book_isbn_key ON book (isbn);
ALTER TABLE book ADD CONSTRAINT book_author_key UNIQUE (author_id);
ALTER TABLE book DROP CONSTRAINT fk_book_author_id;
DROP TABLE IF EXISTS tmp;").unwrap();
        assert_eq!(migration_tables(&dir), vec![Table {
            name: "book".into(),
            columns: vec![column("id", "INTEGER", true), column("name", "TEXT", false), column("isbn", "TEXT", true), column("author_id", "INT8", true)],
        }]);
        std::fs::write(dir.join("3_rename_book.sql"), "ALTER TABLE book RENAME TO books;").unwrap();
        assert_eq!(migration_tables(&dir)[0].name, "books");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn add_columns_migration_declares_the_defaults_the_unique_indexes_and_the_foreign_keys() {
        let fields = vec![
            field("stars", "i32"),
            Field { unique: true, length: Some(20), ..field("isbn", "Option<String>") },
            Field { references: Some(Reference { table: "author".into(), column: "id".into() }), ..field("author_id", "Option<i64>") },
        ];
        let defaults = vec![("stars".to_string(), "0".to_string())];
        assert_eq!(render_add_columns("book", &fields, &defaults, Dialect::Sqlite).unwrap(), "ALTER TABLE book ADD COLUMN stars INTEGER NOT NULL DEFAULT 0;
ALTER TABLE book ADD COLUMN isbn TEXT;
ALTER TABLE book ADD COLUMN author_id INTEGER REFERENCES author (id);
CREATE UNIQUE INDEX book_isbn_key ON book (isbn);
");
        assert_eq!(render_add_columns("book", &fields, &defaults, Dialect::Postgres).unwrap(), "ALTER TABLE book ADD COLUMN stars INT4 NOT NULL DEFAULT 0;
ALTER TABLE book ADD COLUMN isbn VARCHAR(20);
ALTER TABLE book ADD COLUMN author_id INT8;
CREATE UNIQUE INDEX book_isbn_key ON book (isbn);
ALTER TABLE book ADD CONSTRAINT fk_book_author_id FOREIGN KEY (author_id) REFERENCES author (id);
");
        assert_eq!(render_add_columns("book", &fields, &defaults, Dialect::Mysql).unwrap(), "ALTER TABLE book ADD COLUMN stars INT NOT NULL DEFAULT 0;
ALTER TABLE book ADD COLUMN isbn VARCHAR(20);
ALTER TABLE book ADD COLUMN author_id BIGINT;
CREATE UNIQUE INDEX book_isbn_key ON book (isbn);
ALTER TABLE book ADD CONSTRAINT fk_book_author_id FOREIGN KEY (author_id) REFERENCES author (id);
");
        assert!(render_add_columns("book", &[field("tags", "Vec<String>")], &[], Dialect::Sqlite).is_err());
    }

    #[test]
    fn sqlite_refuses_the_columns_it_cannot_add() {
        let author_id = Field { references: Some(Reference { table: "author".into(), column: "id".into() }), ..field("author_id", "i64") };
        assert!(add_column_error(&field("stars", "i32"), None, Dialect::Sqlite).is_some());
        assert!(add_column_error(&field("stars", "i32"), Some("NULL"), Dialect::Sqlite).is_some());
        assert!(add_column_error(&field("stars", "i32"), Some("0"), Dialect::Sqlite).is_none());
        assert!(add_column_error(&field("at", "DateTime<Utc>"), Some("CURRENT_TIMESTAMP"), Dialect::Sqlite).is_some());
        assert!(add_column_error(&field("at", "NaiveDate"), Some("(date('now'))"), Dialect::Sqlite).is_some());
        assert!(add_column_error(&author_id, Some("1"), Dialect::Sqlite).is_some());
        assert!(add_column_error(&Field { ty: "Option<i64>".into(), ..author_id.clone() }, None, Dialect::Sqlite).is_none());
        assert!(add_column_error(&field("stars", "i32"), None, Dialect::Postgres).is_none());
        assert!(add_column_error(&field("at", "DateTime<Utc>"), Some("CURRENT_TIMESTAMP"), Dialect::Mysql).is_none());
    }

    #[test]
    fn read_default_proposes_a_default_or_makes_the_field_optional() {
        let read = |answers: &str, field: &mut Field, flag: Option<&str>, dialect: Dialect| {
            let mut output = Vec::new();
            let default = read_default(&mut answers.as_bytes(), &mut output, field, flag, dialect).unwrap();
            (default, String::from_utf8(output).unwrap())
        };
        let mut stars = field("stars", "i32");
        assert_eq!(read("\n", &mut stars, None, Dialect::Sqlite).0.as_deref(), Some("0"));
        assert_eq!(read("5\n", &mut stars, None, Dialect::Postgres).0.as_deref(), Some("5"));
        assert_eq!(read("", &mut stars, Some("'x'"), Dialect::Postgres).0.as_deref(), Some("'x'"));
        // refused by SQLite, then asked again
        let mut at = field("at", "DateTime<Utc>");
        let (default, output) = read("CURRENT_TIMESTAMP\n\n", &mut at, None, Dialect::Sqlite);
        assert_eq!(default.as_deref(), Some("'1970-01-01 00:00:00'"));
        assert!(output.contains("constant default"));
        assert_eq!(read("\n", &mut at, None, Dialect::Mysql).0.as_deref(), Some("CURRENT_TIMESTAMP(6)"));
        assert_eq!(read("?\n", &mut at, None, Dialect::Postgres).0, None);
        assert_eq!(at.ty, "Option<DateTime<Utc>>");
        // optional fields are not asked
        assert_eq!(read("", &mut at, None, Dialect::Postgres).0, None);
        let mut author_id = Field { references: Some(Reference { table: "author".into(), column: "id".into() }), ..field("author_id", "i64") };
        let (default, output) = read("", &mut author_id, None, Dialect::Sqlite);
        assert_eq!((default, author_id.ty.as_str()), (None, "Option<i64>"));
        assert!(output.contains("nullable"));
    }

    #[test]
    fn read_new_fields_refuses_the_declared_fields() {
        let mut output = Vec::new();
        let declared = vec!["id".to_string(), "title".to_string()];
        let fields = read_new_fields(&mut "title\nstars:i32\n\n".as_bytes(), &mut output, false, &declared, None, None, false).unwrap();
        assert_eq!(fields, vec![field("stars", "i32")]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Field `title` is already declared"));
        assert!(output.contains("the model already declares id, title"));
    }

    // A model file of `fields`, as `generate-model` writes it
    fn model_file(fields: &[Field], graphql: bool, sqlx: bool, timestamps: bool, dialect: Dialect) -> String {
        super::with_header("//", &render_table_model("BookPage", Some("book_page"), false, graphql, sqlx, timestamps, fields, dialect))
    }

    #[test]
    fn added_fields_are_inserted_in_the_model_structs_as_generate_model_declares_them() {
        let title = Field { length: Some(80), ..field("title", "String") };
        let added = [field("stars", "i32"), field("type", "Option<String>")];
        for dialect in [Dialect::Sqlite, Dialect::Postgres, Dialect::Mysql] {
            for (graphql, sqlx, timestamps) in [(false, false, false), (true, true, false), (false, true, true), (true, true, true)] {
                let source = model_file(std::slice::from_ref(&title), graphql, sqlx, timestamps, dialect);
                let all = [title.clone(), added[0].clone(), added[1].clone()];
                assert_eq!(
                    with_added_fields(&source, "BookPage", &added).unwrap(),
                    model_file(&all, graphql, sqlx, timestamps, dialect),
                    "graphql {} sqlx {} timestamps {} {:?}", graphql, sqlx, timestamps, dialect
                );
            }
        }
    }

    #[test]
    fn added_fields_keep_the_code_written_in_the_model() {
        let source = model_file(&[field("title", "String")], false, true, false, Dialect::Sqlite)
            .replace("    pub struct NewBookPage {", "    // created by the form\n    pub struct NewBookPage {")
            .replace("        pub title: String,\n    }\n\n    #[derive(Serialize", "        pub title: String // no comma\n    }\n\n    #[derive(Serialize")
            + "\n    fn hand_written() -> u8 { 1 }\n";
        let patched = with_added_fields(&source, "BookPage", &[field("stars", "i32")]).unwrap();
        assert!(patched.contains("    // created by the form\n    pub struct NewBookPage {\n        pub title: String, // no comma\n        pub stars: i32,\n    }"));
        assert!(patched.ends_with("\n    fn hand_written() -> u8 { 1 }\n"));
        assert!(syn::parse_file(&patched).is_ok());
    }

    #[test]
    fn added_fields_import_their_chrono_types() {
        let published = [field("published", "NaiveDate")];
        // added to the import of the timestamps
        let source = model_file(&[field("title", "String")], false, true, true, Dialect::Postgres);
        let patched = with_added_fields(&source, "BookPage", &published).unwrap();
        assert!(patched.contains("    use chrono::{DateTime, Utc, NaiveDate};\n"));
        // already imported
        let source = model_file(&[field("at", "NaiveDate")], false, true, false, Dialect::Postgres);
        assert_eq!(with_added_fields(&source, "BookPage", &published).unwrap().matches("NaiveDate}").count(), 1);
        // in a new import after the last one
        let source = model_file(&[field("title", "String")], false, true, false, Dialect::Postgres);
        let patched = with_added_fields(&source, "BookPage", &published).unwrap();
        assert!(patched.contains("    use octopux::gen_endpoint;\n    use chrono::{NaiveDate};\n"));
        assert!(syn::parse_file(&patched).is_ok());
    }

    #[test]
    fn model_file_gives_the_declared_fields_and_refuses_the_other_files() {
        let source = model_file(&[field("title", "String")], false, true, true, Dialect::Sqlite);
        assert_eq!(read_model_file(&source, "BookPage"), Ok(ModelFile {
            fields: vec!["id".into(), "title".into(), "created_at".into(), "updated_at".into(), "deleted_at".into()],
            timestamps: true,
            sqlx: true,
        }));
        let source = model_file(&[field("type", "String")], false, false, false, Dialect::Sqlite);
        assert_eq!(read_model_file(&source, "BookPage").map(|m| (m.fields, m.timestamps, m.sqlx)), Ok((vec!["id".into(), "type".into()], false, false)));
        assert!(read_model_file(&source, "Book").unwrap_err().contains("no `Book` struct"));
        assert!(read_model_file("pub struct BookPage {}", "BookPage").unwrap_err().contains("not generated by octopux"));
        let _ = write_source;
    }

    #[test]
    fn add_field_default_requires_migration() {
        let parse = |flags: &[&str]| Opt::from_iter_safe(["octopux", "add-field", "--model", "Book", "--sqlite"].iter().chain(flags));
        assert!(parse(&["--default", "0"]).is_err());
        assert!(parse(&["--foreign-keys"]).is_err());
        assert!(parse(&["--migration", "--default", "0", "--unique"]).is_ok());
        assert!(Opt::from_iter_safe(["octopux", "add-field", "--model", "Book"]).is_err());
    }
}
