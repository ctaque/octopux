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

    // The application state, declared (or re-exported) at the root of the crate
    use crate::AppState;
    use serde::{Serialize, Deserialize};
    use octopux::{
        HttpCreate,
        HttpFindListDelete,
        HttpUpdate,
        SqlxModel,
        SqlxNewModel,
        SqlxUpdatableModel,
        SqlxFilter,
        octopux_info,
        Model,
        NewModel,
        UpdatableModel,
    };
    use chrono::{DateTime, Utc};
    use apistos::ApiComponent;
    use schemars::JsonSchema;
    use octopux::gen_documented_endpoint;
    use async_graphql::{ComplexObject, Context, InputObject, Object, SimpleObject};

    #[derive(Default, Deserialize, JsonSchema, ApiComponent)]
    pub struct FindQuery {}
    #[derive(Default, Deserialize, JsonSchema, ApiComponent, SqlxFilter)]
    #[sqlx_filter(database = "postgres")]
    pub struct ListQuery {
        /// Number of rows to skip
        pub offset: Option<usize>,
        /// Maximum number of rows to return (20 by default, 100 at most)
        pub limit: Option<usize>,
        /// The columns to sort by, separated by commas, a column prefixed with `-` in descending order: `isbn,-uuid`.
        /// Among `uuid`, `isbn`, `created_at`, `updated_at`, `uuid` by default
        #[sqlx_filter(sort = "uuid, isbn, created_at, updated_at")]
        pub sort: Option<String>,
        /// The direction of the columns not prefixed with `-`, `asc` (by default) or `desc`
        #[sqlx_filter(sort_direction)]
        pub order: Option<String>,
    }
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    pub struct DeleteQuery {}
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    pub struct SaveQuery {}
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    pub struct UpdateQuery {}
    pub type Id = uuid::Uuid;

    #[derive(Default, Serialize, Deserialize, JsonSchema, ApiComponent, SimpleObject, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
    #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
    #[sqlx_model(database = "postgres", primary_key = "uuid", timestamps, soft_delete, filter)]
    #[octopux_info(path = "book")]
    #[graphql(complex)]
    pub struct Book {
        pub uuid: Id,
        pub isbn: String,
        pub created_at: Option<DateTime<Utc>>,
        pub updated_at: Option<DateTime<Utc>>,
        pub deleted_at: Option<DateTime<Utc>>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Book", primary_key = "uuid", new_id = "uuid::Uuid::new_v4", timestamps)]
    pub struct NewBook {
        pub isbn: String,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Book, FindQuery, AppState)]
    #[sqlx_model(database = "postgres", primary_key = "uuid", timestamps, soft_delete)]
    pub struct UpdatableBook {
        pub uuid: Id,
        pub isbn: String,
        pub updated_at: Option<DateTime<Utc>>,
    }

    // Registers the documented routes of the book endpoint
    // (octopux `openapi` feature), to mount with `.configure(book::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Book, NewBook, UpdatableBook)(cfg)
    }


    // Fields of the GraphQL Book type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Book {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
    }

    // GraphQL queries of the book model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(book::BookQuery, ...);`
    #[derive(Default)]
    pub struct BookQuery;

    #[Object]
    impl BookQuery {
        async fn book(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Book> {
            find(id, app_state(ctx)?).await
        }

        async fn books(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>, sort: Option<String>, order: Option<String>) -> async_graphql::Result<Vec<Book>> {
            Ok(Book::list(&ListQuery { offset, limit, sort, order }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the book model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(book::BookMutation, ...);`
    #[derive(Default)]
    pub struct BookMutation;

    #[Object]
    impl BookMutation {
        async fn create_book(&self, ctx: &Context<'_>, input: NewBook) -> async_graphql::Result<Book> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_book(&self, ctx: &Context<'_>, input: UpdatableBook) -> async_graphql::Result<Book> {
            let state = app_state(ctx)?;
            let id = input.uuid;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_book(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Book> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the book up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Book> {
        match Book::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    