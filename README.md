
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

With `before_save`, implement `BeforeSave`, e.g. to hash a password. An error answers 500:

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
| `--sqlx`, `--migration`, `--timestamps`, `--openapi`, `--graphql`, database flags, `--output`, `--force` | As for `generate-model` |

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
| `--sqlite`, `--postgres`, `--mysql` | Check the database type of the url |
| `--migrations <dir>` | Build the schema from sqlx migrations instead of a live database |
| `--schema <name>` | Read the tables of this PostgreSQL schema instead of the current one (`public`), with `--postgres` |
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
| [`reverse_engineering`](examples/reverse_engineering) | An API generated by `octopux-reverse` from ~100 tables |

## Issues

- apistos panics with `path name regex`: run `cargo update -p regex`.
- sqlx fails to compile with `strip`, see [#5](https://github.com/ctaque/octopux/issues/5).
