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
    #[octopux_info(path = "accountrole")]
    #[graphql(complex)]
    pub struct Accountrole {
        pub id: Id,
        pub account_id: i64,
        pub role_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Accountrole")]
    pub struct NewAccountrole {
        pub account_id: i64,
        pub role_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Accountrole, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableAccountrole {
        pub id: Id,
        pub account_id: i64,
        pub role_id: i64,
    }

    // Registers the documented routes of the accountrole endpoint
    // (octopux `openapi` feature), to mount with `.configure(accountrole::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Accountrole, NewAccountrole, UpdatableAccountrole)(cfg)
    }


    // Fields of the GraphQL Accountrole type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Accountrole {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
    }

    // GraphQL queries of the accountrole model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(accountrole::AccountroleQuery, ...);`
    #[derive(Default)]
    pub struct AccountroleQuery;

    #[Object]
    impl AccountroleQuery {
        async fn accountrole(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Accountrole> {
            find(id, app_state(ctx)?).await
        }

        async fn accountroles(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Accountrole>> {
            Ok(Accountrole::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the accountrole model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(accountrole::AccountroleMutation, ...);`
    #[derive(Default)]
    pub struct AccountroleMutation;

    #[Object]
    impl AccountroleMutation {
        async fn create_accountrole(&self, ctx: &Context<'_>, input: NewAccountrole) -> async_graphql::Result<Accountrole> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_accountrole(&self, ctx: &Context<'_>, input: UpdatableAccountrole) -> async_graphql::Result<Accountrole> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_accountrole(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Accountrole> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the accountrole up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Accountrole> {
        match Accountrole::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    