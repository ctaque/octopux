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
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    pub struct ListQuery {
        /// Number of rows to skip
        pub offset: Option<usize>,
        /// Maximum number of rows to return (20 by default, 100 at most)
        pub limit: Option<usize>,
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
    #[sqlx_model(database = "postgres")]
    #[octopux_info(path = "journalentry")]
    #[graphql(complex)]
    pub struct Journalentry {
        pub id: Id,
        pub reference: String,
        pub journal_id: i64,
        pub posted_at: DateTime<Utc>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Journalentry")]
    pub struct NewJournalentry {
        pub reference: String,
        pub journal_id: i64,
        pub posted_at: DateTime<Utc>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Journalentry, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableJournalentry {
        pub id: Id,
        pub reference: String,
        pub journal_id: i64,
        pub posted_at: DateTime<Utc>,
    }

    // Registers the documented routes of the journalentry endpoint
    // (octopux `openapi` feature), to mount with `.configure(journalentry::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Journalentry, NewJournalentry, UpdatableJournalentry)(cfg)
    }


    // Fields of the GraphQL Journalentry type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Journalentry {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The journallines of the journalentry, paginated
        async fn journallines(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::journalline::Journalline>> {
            crate::journalentry_journallines::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the journalentry model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(journalentry::JournalentryQuery, ...);`
    #[derive(Default)]
    pub struct JournalentryQuery;

    #[Object]
    impl JournalentryQuery {
        async fn journalentry(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Journalentry> {
            find(id, app_state(ctx)?).await
        }

        async fn journalentries(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Journalentry>> {
            Ok(Journalentry::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the journalentry model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(journalentry::JournalentryMutation, ...);`
    #[derive(Default)]
    pub struct JournalentryMutation;

    #[Object]
    impl JournalentryMutation {
        async fn create_journalentry(&self, ctx: &Context<'_>, input: NewJournalentry) -> async_graphql::Result<Journalentry> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_journalentry(&self, ctx: &Context<'_>, input: UpdatableJournalentry) -> async_graphql::Result<Journalentry> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_journalentry(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Journalentry> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the journalentry up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Journalentry> {
        match Journalentry::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    