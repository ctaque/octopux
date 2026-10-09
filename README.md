
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


# octopux

Generate JSON CRUD endpoints for [Actix Web](https://actix.rs) (v4) from your structs.

- **5 REST routes per model**: find, list, create, update, delete
- **[sqlx](https://docs.rs/sqlx) persistence** (SQLite, PostgreSQL, MySQL), queries written for you
- **OpenAPI** documentation with [apistos](https://docs.rs/apistos) and Swagger UI
- **GraphQL** with [async-graphql](https://docs.rs/async-graphql) and GraphiQL
- **Has-many and many-to-many relations**
- **PostGIS** geometries as GeoJSON, with spatial filters
- **pgvector** embeddings, with similarity search
- **A CLI** generating models, relations and SQL migrations, even from an existing database

## Table of contents

- [Installation](#installation)
- [Quick start](#quick-start)
- [How it works](#how-it-works)
- [Guides](#guides)
- [CLI](#cli)
- [Reverse engineering a database](#reverse-engineering-a-database)
- [Examples](#examples)
- [Issues](#issues)

## Installation

```bash
cargo install octopux-cli   # the `octopux` CLI
cargo install sqlx-cli      # optional, for `sqlx migrate run`
```

The library, in your crate:

```bash
cargo add octopux --features sqlx,openapi
cargo add actix-web serde --features serde/derive
```

| Feature | Enables | Also add |
| --- | --- | --- |
| `sqlx` | The sqlx derives (`SqlxModel`, `SqlxFilter`...) | `sqlx --features runtime-tokio,sqlite,macros,migrate,chrono` |
| `openapi` | The documented routes | `apistos --features chrono,swagger-ui` and `schemars --rename schemars --package apistos-schemars` |
| `postgis` | The PostGIS geometries (`octopux::postgis`), see [PostGIS](#postgis) | `postgres` in the sqlx features |
| `pgvector` | The pgvector vectors (`octopux::pgvector`), see [pgvector](#pgvector) | `postgres` in the sqlx features |
| `graphql` | With `postgis` or `pgvector`, the geometries and the vectors as GraphQL scalars | `async-graphql` |

Add `chrono --features serde` when a model has date fields or uses `--timestamps`.

> `octopux --bootstrap` offers to install all these dependencies for you.

## Quick start

```bash
cargo init my-api && cd my-api
octopux --bootstrap
octopux generate-model --name Project --fields --sqlx --migration --timestamps --openapi --sqlite
```

1. `--bootstrap` asks for the database, OpenAPI and GraphQL (`octopux --bootstrap --sqlite --openapi` answers them with flags), then writes `src/main.rs` (an actix server on `127.0.0.1:8085`, a SQLite pool on `data.db`) and `src/helpers.rs` (the `AppState` holding the pool).
2. `generate-model` asks for the fields, then writes `src/project.rs` and the migration creating the `project` table.
3. In `src/main.rs`, declare the model with `mod project;`, mount it with `.configure(project::configure)` in the `v1` scope, and uncomment `sqlx::migrate!().run(&pool)`.

```bash
cargo run
```

| Route | Action |
| --- | --- |
| `GET /v1/project` | List a page of projects |
| `GET /v1/project/{id}` | Find a project |
| `POST /v1/project` | Create a project |
| `PUT /v1/project/{id}` | Update a project |
| `DELETE /v1/project/{id}` | Delete a project |

Swagger UI is served on `/swagger` ([screenshot](openapi.png)), the OpenAPI document on `/openapi.json`.

## How it works

A resource is made of three structs. Each derives the HTTP handlers of its routes and implements a trait holding the logic, written by hand or derived with sqlx:

| Struct | HTTP derive | Trait | sqlx derive | Routes |
| --- | --- | --- | --- | --- |
| `Project` | `HttpFindListDelete` | `Model` (`find`, `list`, `delete`) | `SqlxModel` | `GET /project/{id}`, `GET /project`, `DELETE /project/{id}` |
| `NewProject` | `HttpCreate` | `NewModel` (`save`) | `SqlxNewModel` | `POST /project` |
| `UpdatableProject` | `HttpUpdate` | `UpdatableModel` (`update`) | `SqlxUpdatableModel` | `PUT /project/{id}` |

Without OpenAPI, mount the routes with `gen_endpoint!(Project, NewProject, UpdatableProject)`.

## Guides

### sqlx options

The sqlx derives take a `#[sqlx_model(...)]` attribute:

| Key | Description |
| --- | --- |
| `database` | **Required.** `"sqlite"`, `"postgres"` or `"mysql"` |
| `model` | **Required on `SqlxNewModel`.** The model returned by `save` |
| `table` | The table, the lowercase model name by default |
| `pool` | The field of `AppState` holding the pool, `pool` by default |
| `timestamps` | Fill `created_at` and `updated_at` automatically |
| `soft_delete` | `delete` sets `deleted_at` instead of removing the row, which is then hidden |
| `default_limit`, `max_limit` | The page size of `list` |
| `filter` | `list` applies the filters and sort of its query, see [`SqlxFilter`](#filtering-and-sorting) |
| `before_save` | Transform the payload before writing it, see below |

### Transforming the payload

With `before_save`, implement `BeforeSave`, e.g. to hash a password. An error aborts the query, see [Errors](#errors) for its status:

```rust
#[derive(Serialize, Deserialize, HttpCreate, SqlxNewModel)]
#[http_create(SaveQuery, AppState)]
#[sqlx_model(database = "postgres", model = "User", before_save)]
pub struct NewUser {
    pub email: String,
    pub password: String,
}

#[async_trait]
impl BeforeSave<AppState> for NewUser {
    async fn before_save(mut self: Self, _state: &AppState) -> Result<Self> {
        self.password = hash(&self.password)?;
        Ok(self)
    }
}
```

### Filtering and sorting

`SqlxFilter` turns the `Option` fields of the list query into `WHERE` conditions. The field name gives the column and the operator (suffixes `_ne`, `_gt`, `_gte`, `_lt`, `_lte`, `_like`):

```rust
#[derive(Deserialize, SqlxFilter)]
#[sqlx_filter(database = "postgres")]
pub struct ListQuery {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    pub name: Option<String>,      // ?name=foo        → name = 'foo'
    pub price_gte: Option<i32>,    // ?price_gte=10    → price >= 10
    #[sqlx_filter(column = "name", op = "like")]
    pub q: Option<String>,         // ?q=alpha%        → name LIKE 'alpha%'
    #[sqlx_filter(sort = "name, price")]
    pub sort: Option<String>,      // ?sort=-price,name (only these columns)
    #[sqlx_filter(sort_direction)]
    pub order: Option<String>,     // ?order=desc
}
```

Add `filter` to the `#[sqlx_model]` of the model to apply it in `list`. Values are always bound, never written in the SQL.

### Relations

A relation lists the children of a model on its own paginated route, e.g. `GET /v1/project/{id}/books`. Generate it with [`generate-relation`](#cli), or implement `HasMany` by hand:

```rust
pub struct ProjectBooks;

#[async_trait]
impl HasMany for ProjectBooks {
    type Parent = Project;
    type Id = Id;
    type Query = ProjectBooksQuery;
    type Result = Vec<Book>;
    type State = AppState;
    const RELATION: &'static str = "books";

    async fn list_related(id: Id, query: &ProjectBooksQuery, state: &AppState) -> Result<Option<Vec<Book>>> {
        // `None` answers 404 when the project does not exist
    }
}
```

### Errors

Errors are answered in JSON, with a stable `code` to match on:

```json
{ "code": "ENTITY_NOT_FOUND", "message": "no entity has this id" }
```

| Status | `code` | When |
| --- | --- | --- |
| 400 | `ID_MISMATCH` | The `id` of the `PUT` payload differs from the `id` of the path |
| 400 | `BAD_REQUEST` | Invalid sort, `NOT NULL` or `CHECK` constraint violated, or `Error::BadRequest` |
| 404 | `ENTITY_NOT_FOUND` | No entity has this id (`sqlx::Error::RowNotFound`, or `Error::NotFound`) |
| 409 | `CONFLICT` | Unique or foreign key constraint violated, or `Error::Conflict` |
| 500 | `INTERNAL_ERROR` | Any other error: its message is logged with the [`log`](https://docs.rs/log) crate, not sent |

Return an `octopux::Error` from a model, a relation or a `BeforeSave` hook to answer a client error:

```rust
async fn before_save(mut self: Self, _state: &AppState) -> Result<Self> {
    if !self.email.contains('@') {
        return Err(octopux::Error::BadRequest("the email is invalid".into()).into());
    }
    Ok(self)
}
```

With the `openapi` feature, every error response is documented with the `OctopuxError` schema.

### GraphQL

Add `--graphql` to the three CLI commands to serve the models on `/graphql` (GraphiQL on `GET /graphql`), next to the REST routes:

```bash
octopux --bootstrap --sqlite --graphql
octopux generate-model --name Author --fields --sqlx --migration --graphql --sqlite
octopux generate-model --name Book --fields --sqlx --migration --foreign-keys --graphql --sqlite
octopux generate-relation --parent Author --child Book --foreign-key=author_id --sqlx --migration --graphql --sqlite
```

Each model gets the `author(id)` and `authors(offset, limit)` queries and the `createAuthor`, `updateAuthor` and `deleteAuthor` mutations, merged into the schema of `src/main.rs`. Relations become fields:

```graphql
query {
  authors(limit: 10) {
    id
    name
    books(limit: 5) { id title }
  }
}
```

### PostGIS

With the `postgis` feature, the `geometry` and `geography` columns of PostgreSQL are fields of the sqlx models. They are read and written as EWKB, with their SRID, and sent to and received from the client as GeoJSON geometries:

```rust
use octopux::postgis::{self, Point, Polygon};

#[derive(Serialize, Deserialize, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
#[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
#[sqlx_model(database = "postgres", filter)]
#[octopux_info(path = "place")]
pub struct Place {
    pub id: Id,
    pub name: String,
    pub location: Point,                  // geometry(Point, 4326) NOT NULL
    pub area: Option<Polygon>,            // geometry(Polygon, 4326)
    pub zone: Option<postgis::Geometry>,  // any kind of geometry
}
```

```json
{ "id": 1, "name": "Tour Eiffel", "location": { "type": "Point", "coordinates": [2.2945, 48.8584] }, "area": null, "zone": null }
```

`Point`, `LineString`, `Polygon`, `MultiPoint`, `MultiLineString`, `MultiPolygon` and `GeometryCollection` restrict a field to one kind of geometry, `Geometry` accepts any. They hold a [`geo-types`](https://docs.rs/geo-types) geometry (`Deref`, `into_inner`). A payload of another kind, or with a Z or M coordinate, answers 400 `BAD_REQUEST`.

GeoJSON coordinates are WGS 84 `[longitude, latitude]`: a deserialized geometry gets the SRID 4326 (`postgis::WGS84`). Build the geometry with `Geometry::new(geometry, srid)` for a column of another SRID.

`SqlxFilter` filters a list with the spatial operators of PostGIS, given by `op`:

```rust
#[derive(Deserialize, SqlxFilter)]
#[sqlx_filter(database = "postgres")]
pub struct ListQuery {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    #[sqlx_filter(column = "location", op = "intersects")]
    pub bbox: Option<postgis::Bbox>,      // ?bbox=2.28,48.85,2.30,48.87
    #[sqlx_filter(column = "location", op = "dwithin")]
    pub near: Option<postgis::Near>,      // ?near=2.2945,48.8584,800
    #[sqlx_filter(column = "area", op = "contains")]
    pub point: Option<postgis::Point>,    // ?point={"type":"Point","coordinates":[2.2945,48.8584]}
}
```

| `op` | Condition | Value |
| --- | --- | --- |
| `intersects` | `ST_Intersects(column, value)` | A `Bbox` (`west,south,east,north`) or a GeoJSON geometry |
| `within` | `ST_Within(column, value)`, the column inside the value | Same, `geometry` columns only |
| `contains` | `ST_Contains(column, value)` | Same, `geometry` columns only |
| `dwithin` | `ST_DWithin(column::geography, point::geography, meters)` | A `Near` (`longitude,latitude,meters`) |

`Bbox` and `Near` are in WGS 84, the column too. Index the columns with GiST, and `(column::geography)` for `dwithin` on a `geometry` column:

```sql
CREATE INDEX place_location_idx ON place USING GIST (location);
CREATE INDEX place_location_geography_idx ON place USING GIST ((location::geography));
```

With the `graphql` feature too, the geometries are async-graphql scalars, the same GeoJSON: `GeoJsonPoint`, `GeoJsonPolygon`..., `GeoJsonGeometry` for any kind. With `openapi`, they are documented as GeoJSON geometries.

The CLI proposes the geometries in the field types with `--postgres` (`location:postgis::Point`): the migration creates the `postgis` extension, `geometry(Point, 4326)` columns and their GiST indexes. `octopux-reverse` maps the `geometry` and `geography` columns to these types, except the Z and M kinds. See the [`postgis`](examples/postgis) example.

### pgvector

With the `pgvector` feature, the `vector`, `halfvec` and `sparsevec` columns of [pgvector](https://github.com/pgvector/pgvector) are fields of the sqlx models, read and written in the binary format of pgvector and sent to and received from the client as JSON:

```rust
use octopux::pgvector::{self, Vector};

#[derive(Serialize, Deserialize, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
#[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
#[sqlx_model(database = "postgres", filter)]
#[octopux_info(path = "article")]
pub struct Article {
    pub id: Id,
    pub title: String,
    pub embedding: Vector,                          // vector(1536) NOT NULL
    pub small: Option<pgvector::HalfVector>,        // halfvec(3072)
    pub keywords: Option<pgvector::SparseVector>,   // sparsevec(30522)
}
```

```json
{ "id": 1, "title": "Ramen in Tokyo", "embedding": [0.7, 0.0, 0.05, 0.7], "small": null, "keywords": { "dimensions": 8, "indices": [4, 6], "values": [1.0, 0.5] } }
```

`Vector` and `HalfVector` are arrays of numbers (`Deref` to `Vec<f32>`); a `HalfVector` is rounded to half precision in the database, which halves the column and its index. A `SparseVector` is its dimensions and its non-zero values with their indices, from 0. In a query string, they are the literals of pgvector: `[0.7,0,0.05,0.7]` (or `0.7,0,0.05,0.7`), and `{5:1,7:0.5}/8` with the indices from 1. A vector with no dimension, a value that is not finite, or a vector of another size than its column (`vector(1536)`) answers 400 `BAD_REQUEST`.

`SqlxFilter` orders a list by the distance of a column to the vector of the query, nearest first, with `op = "nearest"`; `limit` keeps the `k` nearest rows:

```rust
#[derive(Deserialize, SqlxFilter)]
#[sqlx_filter(database = "postgres")]
pub struct ListQuery {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    pub category: Option<String>,
    #[sqlx_filter(column = "embedding", op = "nearest")]
    pub near: Option<pgvector::Vector>,           // ?near=[0.8,0,0,0.6]&limit=5
    #[sqlx_filter(column = "keywords", op = "nearest", distance = "inner_product")]
    pub keywords: Option<pgvector::SparseVector>, // ?keywords={5:1,6:1}/8
    #[sqlx_filter(sort = "title")]
    pub sort: Option<String>,                     // orders the rows at the same distance
}
```

| `distance` | Order | HNSW operator class |
| --- | --- | --- |
| `cosine` (default) | `column <=> value` | `vector_cosine_ops` |
| `l2` | `column <-> value` | `vector_l2_ops` |
| `inner_product` | `column <#> value`, the largest inner product first | `vector_ip_ops` |
| `l1` | `column <+> value` | `vector_l1_ops` |

The conditions of the other fields still apply, and the distance orders the rows before the columns of the `sort` field. Index the column with HNSW and the operator class of the distance (`halfvec_cosine_ops`, `sparsevec_cosine_ops`... for the other types), up to 2000 dimensions for a `vector`, 4000 for a `halfvec`:

```sql
CREATE INDEX article_embedding_idx ON article USING hnsw (embedding vector_cosine_ops);
```

With the `graphql` feature too, the vectors are the async-graphql scalars `Vector`, `HalfVector` and `SparseVector`, the same JSON or the literal of pgvector. With `openapi`, they are documented as arrays and objects.

The CLI proposes the vectors in the field types with `--postgres` (`embedding:pgvector::Vector`) and asks their dimensions: the migration creates the `vector` extension, `vector(1536)` columns and their HNSW indexes for the cosine distance. `octopux-reverse` maps the `vector`, `halfvec` and `sparsevec` columns to these types. See the [`pgvector`](examples/pgvector) example.

## CLI

Files are written in `src` when it exists, in the working directory otherwise.

| Command | Does |
| --- | --- |
| `octopux --bootstrap [--sqlite\|--postgres\|--mysql] [--openapi] [--graphql]` | Write `src/main.rs` and `src/helpers.rs`, and offer to install the dependencies, asking for the options not given without a database flag |
| `octopux generate-model --name Project --sqlite\|--postgres\|--mysql [OPTIONS]` | Write `project.rs` (and its migration) |
| `octopux generate-relation --parent Project --child Book --sqlite\|--postgres\|--mysql [OPTIONS]` | Write `project_books.rs`, the `books` of a project |
| `octopux add-field --model Project --sqlite\|--postgres\|--mysql [OPTIONS]` | Add fields to `project.rs` (and the migration adding their columns), keeping the code written in it |
| `octopux schema [OPTIONS]` | Draw the tables created by the migrations, their keys and relations |

### generate-model

| Option | Description |
| --- | --- |
| `--fields` | Ask interactively for the fields |
| `--sqlx` | Implement the queries with sqlx |
| `--migration` | Create the SQL migration of the table |
| `--foreign-keys` | Ask which table and column each field references |
| `--unique` | Ask whether each field is unique |
| `--timestamps` | Add `created_at`, `updated_at`, `deleted_at`, with soft delete |
| `--sqlite`, `--postgres`, `--mysql` | **Required.** The target database |
| `--openapi`, `--graphql` | Document the routes / serve the model with GraphQL |
| `--table` | The table name, the snake_case model name by default |
| `--schema <name>` | With `--postgres`, the schema of the table: the queries and the migration use `app.project`, the migration creates the schema if missing |
| `--output` | The folder of the file (`--output=src/models`) |
| `--force` | Overwrite an existing file |

When asked for the fields, type `name:type` (e.g. `title:String`, or `stars:2` to pick from the menu). A trailing `?` makes the field optional (`stars:i32?`), `-` removes the last one, and an empty name ends the input:

```
$ octopux generate-model --name Project --fields --sqlite
? Field 1 name › title:String
  ✔ title: String
? Field 2 name › stars:i32?
  ✔ stars: Option<i32> (nullable)
? Field 3 name ›
```

### generate-relation

| Option | Description |
| --- | --- |
| `--parent`, `--child` | The two models |
| `--foreign-key` | The column of the child referencing the parent, `project_id` by default |
| `--through` | Many-to-many: the join model |
| `--name` | The route segment, `books` by default |
| `--sqlx`, `--migration`, `--timestamps`, `--openapi`, `--graphql`, database flags, `--schema`, `--output`, `--force` | As for `generate-model` |

```bash
# one-to-many: GET /project/{id}/books
octopux generate-relation --parent Project --child Book --sqlx --openapi --sqlite

# many-to-many through ProjectCategory: GET /project/{id}/categories
octopux generate-relation --parent Project --child Category --through ProjectCategory --sqlx --openapi --sqlite
```

Declare the file with `mod project_books;` and mount it with `.configure(project_books::configure)`.

### add-field

Asks for fields, as `generate-model --fields`, and inserts them in the `Project`, `NewProject` and `UpdatableProject` structs of `project.rs`, before the timestamps. The rest of the file is left as is, unlike `generate-model --force`. The sqlx derives, GraphQL and OpenAPI take the new fields from the structs.

| Option | Description |
| --- | --- |
| `--model` | The model, `Project` |
| `--migration` | Create the `ALTER TABLE ... ADD COLUMN` migration |
| `--foreign-keys`, `--unique` | As for `generate-model` |
| `--default` | The SQL default of the new non-optional columns on the existing rows (`--default "'none'"`), asked for each of them otherwise |
| `--sqlite`, `--postgres`, `--mysql` | **Required.** The target database |
| `--schema <name>` | With `--postgres`, the schema of the table: the migration alters `app.project` |
| `--output` | The folder of the file |

A non-optional column needs a default for the rows already in the table, enter `?` instead to make the field optional. SQLite only adds a foreign key column when it is nullable, and only with a constant default (no `CURRENT_TIMESTAMP`).

```
$ octopux add-field --model Project --migration --sqlite
? Field 1 name › stars:i32
  ✔ stars: i32
? Field 2 name ›
? Default of `stars` for the existing rows › (SQL value, `?` makes the field optional) [0]
```

### schema

Displays the current SQL schema in our terminal.

In a terminal, a filter input above the tables keeps the ones whose table, column or constraint name contains what is typed; keywords separated by `,` or `|` add up (`book, page` or `book|page`). The arrows and the pages scroll, `Esc` quits. When piped, all the tables are drawn.

| Option | Description |
| --- | --- |
| `--migrations` | The migrations folder, `./migrations` by default |

```
$ octopux schema | cat
┌──────────────────────────┐    ┌───────────────────────────────┐
│ author                   │    │ book                          │
├──────────────────────────┤    ├───────────────────────────────┤
│ PK id   INTEGER          │    │ PK id        INTEGER          │
│    name TEXT    not null │    │    title     TEXT    not null │
└──────────────────────────┘    │    author_id INTEGER not null │
                                └───────────────────────────────┘


┌───────────────────────────────────┐
│ page                              │
├───────────────────────────────────┤
│ PK id          INTEGER            │
│    page_number INTEGER   not null │
│ FK book_id     INTEGER   not null │ ──▶ book.id
│    contents    TEXT      not null │
│    created_at  DATETIME?          │
│    updated_at  DATETIME?          │
│    deleted_at  DATETIME?          │
└───────────────────────────────────┘


Relations
  book  1──N  page  page.book_id

PK primary key · FK foreign key · uniq unique · ? nullable
```

## Reverse engineering a database

`octopux-reverse` reads an existing database and generates the matching models and relations, by calling the `octopux` CLI for you.

```bash
cargo install octopux-reverse

# print the commands to review them, then run them
octopux-reverse --database-url postgres://localhost/my_db --openapi > reverse.sh
sh reverse.sh

# or run them directly
octopux-reverse --database-url postgres://localhost/my_db --openapi --run
```

What you get:

- **One model per table** with an `id` primary key (`books` gives `Book`), one field per column. Nullable columns become `Option<T>`.
- **One relation per foreign key** to another table's `id` (`GET /author/{id}/books`), and **many-to-many relations** for join tables.
- `--timestamps` when the table has `created_at`, `updated_at` and `deleted_at` columns.
- At the end, the `mod` and `.configure` lines to paste into `src/main.rs`.

Columns that cannot be mapped to a Rust type are skipped with a warning.

| Option | Description |
| --- | --- |
| `--database-url` | The database to read, `DATABASE_URL` by default |
| `--migrations <dir>` | Build the schema from sqlx migrations instead of a live database |
| `--schema <name>` | Read the tables of this PostgreSQL schema instead of the current one (`public`). Needs a DATABASE_URL or --database-url with `postgresql:` conenction string scheme |
| `--tables`, `--exclude` | Only read, or skip, these tables (comma separated) |
| `--no-relations` | Only generate the models |
| `--run` | Run the commands instead of printing them |
| `--openapi`, `--graphql`, `--output`, `--force` | Passed to `octopux` |

## Examples

Runnable projects in [`examples`](examples), with [Hoppscotch](https://hoppscotch.io/) request collections:

| Example | Shows |
| --- | --- |
| [`simple`](examples/simple) | Traits implemented by hand, with `gen_endpoint!` |
| [`pagination`](examples/pagination) | A paginated `list` |
| [`openapi`](examples/openapi) | sqlx models with Swagger UI |
| [`graphql`](examples/graphql) | Authors, books and pages with GraphQL and relations |
| [`graphql_auth`](examples/graphql_auth) | GraphQL with JWT authentication |
| [`relations_filters_sort`](examples/relations_filters_sort) | Relations, filters and sort (PostgreSQL) |
| [`postgis`](examples/postgis) | PostGIS points and polygons as GeoJSON, and a `nearby` route with `ST_DWithin` |
| [`pgvector`](examples/pgvector) | Embeddings and sparse vectors, a similarity search in the list, and a `similar` route |
| [`recipes`](examples/recipes) | A semantic search over 340 recipes, with embeddings computed locally by fastembed |
| [`reverse_engineering`](examples/reverse_engineering) | An API generated by `octopux-reverse` from ~100 tables |

## Issues

- apistos panics with `path name regex`: run `cargo update -p regex`.
- sqlx fails to compile with `strip`, see [#5](https://github.com/ctaque/octopux/issues/5).
