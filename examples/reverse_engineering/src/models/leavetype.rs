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
    #[octopux_info(path = "leavetype")]
    #[graphql(complex)]
    pub struct Leavetype {
        pub id: Id,
        pub name: String,
        pub paid: bool,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Leavetype")]
    pub struct NewLeavetype {
        pub name: String,
        pub paid: bool,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Leavetype, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableLeavetype {
        pub id: Id,
        pub name: String,
        pub paid: bool,
    }

    // Registers the documented routes of the leavetype endpoint
    // (octopux `openapi` feature), to mount with `.configure(leavetype::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Leavetype, NewLeavetype, UpdatableLeavetype)(cfg)
    }


    // Fields of the GraphQL Leavetype type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Leavetype {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The leaverequests of the leavetype, paginated
        async fn leaverequests(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::leaverequest::Leaverequest>> {
            crate::leavetype_leaverequests::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the leavetype model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(leavetype::LeavetypeQuery, ...);`
    #[derive(Default)]
    pub struct LeavetypeQuery;

    #[Object]
    impl LeavetypeQuery {
        async fn leavetype(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Leavetype> {
            find(id, app_state(ctx)?).await
        }

        async fn leavetypes(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Leavetype>> {
            Ok(Leavetype::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the leavetype model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(leavetype::LeavetypeMutation, ...);`
    #[derive(Default)]
    pub struct LeavetypeMutation;

    #[Object]
    impl LeavetypeMutation {
        async fn create_leavetype(&self, ctx: &Context<'_>, input: NewLeavetype) -> async_graphql::Result<Leavetype> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_leavetype(&self, ctx: &Context<'_>, input: UpdatableLeavetype) -> async_graphql::Result<Leavetype> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_leavetype(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Leavetype> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the leavetype up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Leavetype> {
        match Leavetype::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    