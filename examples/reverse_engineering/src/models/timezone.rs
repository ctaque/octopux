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
    #[octopux_info(path = "timezone")]
    #[graphql(complex)]
    pub struct Timezone {
        pub id: Id,
        pub name: String,
        pub utc_offset: i32,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Timezone")]
    pub struct NewTimezone {
        pub name: String,
        pub utc_offset: i32,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Timezone, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableTimezone {
        pub id: Id,
        pub name: String,
        pub utc_offset: i32,
    }

    // Registers the documented routes of the timezone endpoint
    // (octopux `openapi` feature), to mount with `.configure(timezone::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Timezone, NewTimezone, UpdatableTimezone)(cfg)
    }


    // Fields of the GraphQL Timezone type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Timezone {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The accounts of the timezone, paginated
        async fn accounts(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::account::Account>> {
            super::timezone_accounts::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the timezone model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(timezone::TimezoneQuery, ...);`
    #[derive(Default)]
    pub struct TimezoneQuery;

    #[Object]
    impl TimezoneQuery {
        async fn timezone(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Timezone> {
            find(id, app_state(ctx)?).await
        }

        async fn timezones(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Timezone>> {
            Ok(Timezone::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the timezone model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(timezone::TimezoneMutation, ...);`
    #[derive(Default)]
    pub struct TimezoneMutation;

    #[Object]
    impl TimezoneMutation {
        async fn create_timezone(&self, ctx: &Context<'_>, input: NewTimezone) -> async_graphql::Result<Timezone> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_timezone(&self, ctx: &Context<'_>, input: UpdatableTimezone) -> async_graphql::Result<Timezone> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_timezone(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Timezone> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the timezone up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Timezone> {
        match Timezone::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    