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
    #[octopux_info(path = "returnrequest")]
    #[graphql(complex)]
    pub struct Returnrequest {
        pub id: Id,
        pub reference: String,
        pub sales_order_id: i64,
        pub reason: String,
        pub status: String,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Returnrequest")]
    pub struct NewReturnrequest {
        pub reference: String,
        pub sales_order_id: i64,
        pub reason: String,
        pub status: String,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Returnrequest, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableReturnrequest {
        pub id: Id,
        pub reference: String,
        pub sales_order_id: i64,
        pub reason: String,
        pub status: String,
    }

    // Registers the documented routes of the returnrequest endpoint
    // (octopux `openapi` feature), to mount with `.configure(returnrequest::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Returnrequest, NewReturnrequest, UpdatableReturnrequest)(cfg)
    }


    // Fields of the GraphQL Returnrequest type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Returnrequest {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The returnitems of the returnrequest, paginated
        async fn returnitems(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::returnitem::Returnitem>> {
            crate::returnrequest_returnitems::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the returnrequest model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(returnrequest::ReturnrequestQuery, ...);`
    #[derive(Default)]
    pub struct ReturnrequestQuery;

    #[Object]
    impl ReturnrequestQuery {
        async fn returnrequest(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Returnrequest> {
            find(id, app_state(ctx)?).await
        }

        async fn returnrequests(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Returnrequest>> {
            Ok(Returnrequest::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the returnrequest model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(returnrequest::ReturnrequestMutation, ...);`
    #[derive(Default)]
    pub struct ReturnrequestMutation;

    #[Object]
    impl ReturnrequestMutation {
        async fn create_returnrequest(&self, ctx: &Context<'_>, input: NewReturnrequest) -> async_graphql::Result<Returnrequest> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_returnrequest(&self, ctx: &Context<'_>, input: UpdatableReturnrequest) -> async_graphql::Result<Returnrequest> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_returnrequest(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Returnrequest> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the returnrequest up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Returnrequest> {
        match Returnrequest::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    