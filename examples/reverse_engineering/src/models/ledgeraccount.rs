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
    #[octopux_info(path = "ledgeraccount")]
    #[graphql(complex)]
    pub struct Ledgeraccount {
        pub id: Id,
        pub code: String,
        pub name: String,
        pub parent_id: Option<i64>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Ledgeraccount")]
    pub struct NewLedgeraccount {
        pub code: String,
        pub name: String,
        pub parent_id: Option<i64>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Ledgeraccount, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableLedgeraccount {
        pub id: Id,
        pub code: String,
        pub name: String,
        pub parent_id: Option<i64>,
    }

    // Registers the documented routes of the ledgeraccount endpoint
    // (octopux `openapi` feature), to mount with `.configure(ledgeraccount::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Ledgeraccount, NewLedgeraccount, UpdatableLedgeraccount)(cfg)
    }


    // Fields of the GraphQL Ledgeraccount type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Ledgeraccount {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The ledgeraccounts of the ledgeraccount, paginated
        async fn ledgeraccounts(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::ledgeraccount::Ledgeraccount>> {
            crate::ledgeraccount_ledgeraccounts::resolve(ctx, self.id, offset, limit).await
        }
        /// The journallines of the ledgeraccount, paginated
        async fn journallines(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::journalline::Journalline>> {
            crate::ledgeraccount_journallines::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the ledgeraccount model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(ledgeraccount::LedgeraccountQuery, ...);`
    #[derive(Default)]
    pub struct LedgeraccountQuery;

    #[Object]
    impl LedgeraccountQuery {
        async fn ledgeraccount(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Ledgeraccount> {
            find(id, app_state(ctx)?).await
        }

        async fn ledgeraccounts(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Ledgeraccount>> {
            Ok(Ledgeraccount::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the ledgeraccount model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(ledgeraccount::LedgeraccountMutation, ...);`
    #[derive(Default)]
    pub struct LedgeraccountMutation;

    #[Object]
    impl LedgeraccountMutation {
        async fn create_ledgeraccount(&self, ctx: &Context<'_>, input: NewLedgeraccount) -> async_graphql::Result<Ledgeraccount> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_ledgeraccount(&self, ctx: &Context<'_>, input: UpdatableLedgeraccount) -> async_graphql::Result<Ledgeraccount> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_ledgeraccount(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Ledgeraccount> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the ledgeraccount up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Ledgeraccount> {
        match Ledgeraccount::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    