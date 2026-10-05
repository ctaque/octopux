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
    use crate::shared::AppState;
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
    use octopux::gen_documented_endpoint;
    use schemars::JsonSchema;
    use apistos::ApiComponent;
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
    #[sqlx_model(database = "postgres", table = "my_model", timestamps, soft_delete)]
    #[octopux_info(path = "mymodel")]
    #[graphql(complex)]
    pub struct MyModel {
        pub id: Id,
        pub password: String,
        pub created_at: Option<DateTime<Utc>>,
        pub updated_at: Option<DateTime<Utc>>,
        pub deleted_at: Option<DateTime<Utc>>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "MyModel", table = "my_model", timestamps, before_save)]
    pub struct NewMyModel {
        pub password: String,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, MyModel, FindQuery, AppState)]
    #[sqlx_model(database = "postgres", table = "my_model", timestamps, soft_delete, before_save)]
    pub struct UpdatableMyModel {
        pub id: Id,
        pub password: String,
        pub updated_at: Option<DateTime<Utc>>,
    }

    // Registers the documented routes of the mymodel endpoint
    // (octopux `openapi` feature), to mount with `.configure(my_model::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(MyModel, NewMyModel, UpdatableMyModel)(cfg)
    }


    // Fields of the GraphQL MyModel type resolved by functions, its has-many relations
    #[ComplexObject]
    impl MyModel {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
    }

    // GraphQL queries of the mymodel model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(my_model::MyModelQuery, ...);`
    #[derive(Default)]
    pub struct MyModelQuery;

    #[Object]
    impl MyModelQuery {
        async fn my_model(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<MyModel> {
            find(id, app_state(ctx)?).await
        }

        async fn my_models(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<MyModel>> {
            Ok(MyModel::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the mymodel model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(my_model::MyModelMutation, ...);`
    #[derive(Default)]
    pub struct MyModelMutation;

    #[Object]
    impl MyModelMutation {
        async fn create_my_model(&self, ctx: &Context<'_>, input: NewMyModel) -> async_graphql::Result<MyModel> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_my_model(&self, ctx: &Context<'_>, input: UpdatableMyModel) -> async_graphql::Result<MyModel> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_my_model(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<MyModel> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the mymodel up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<MyModel> {
        match MyModel::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    
