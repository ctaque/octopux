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
    use chrono::{NaiveDate};
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
    #[octopux_info(path = "contract")]
    #[graphql(complex)]
    pub struct Contract {
        pub id: Id,
        pub reference: String,
        pub employee_id: i64,
        pub start_date: NaiveDate,
        pub end_date: Option<NaiveDate>,
        pub salary: f64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Contract")]
    pub struct NewContract {
        pub reference: String,
        pub employee_id: i64,
        pub start_date: NaiveDate,
        pub end_date: Option<NaiveDate>,
        pub salary: f64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Contract, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableContract {
        pub id: Id,
        pub reference: String,
        pub employee_id: i64,
        pub start_date: NaiveDate,
        pub end_date: Option<NaiveDate>,
        pub salary: f64,
    }

    // Registers the documented routes of the contract endpoint
    // (octopux `openapi` feature), to mount with `.configure(contract::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Contract, NewContract, UpdatableContract)(cfg)
    }


    // Fields of the GraphQL Contract type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Contract {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
    }

    // GraphQL queries of the contract model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(contract::ContractQuery, ...);`
    #[derive(Default)]
    pub struct ContractQuery;

    #[Object]
    impl ContractQuery {
        async fn contract(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Contract> {
            find(id, app_state(ctx)?).await
        }

        async fn contracts(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Contract>> {
            Ok(Contract::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the contract model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(contract::ContractMutation, ...);`
    #[derive(Default)]
    pub struct ContractMutation;

    #[Object]
    impl ContractMutation {
        async fn create_contract(&self, ctx: &Context<'_>, input: NewContract) -> async_graphql::Result<Contract> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_contract(&self, ctx: &Context<'_>, input: UpdatableContract) -> async_graphql::Result<Contract> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_contract(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Contract> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the contract up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Contract> {
        match Contract::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    