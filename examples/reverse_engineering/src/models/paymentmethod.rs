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
    #[octopux_info(path = "paymentmethod")]
    #[graphql(complex)]
    pub struct Paymentmethod {
        pub id: Id,
        pub code: String,
        pub name: String,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Paymentmethod")]
    pub struct NewPaymentmethod {
        pub code: String,
        pub name: String,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Paymentmethod, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatablePaymentmethod {
        pub id: Id,
        pub code: String,
        pub name: String,
    }

    // Registers the documented routes of the paymentmethod endpoint
    // (octopux `openapi` feature), to mount with `.configure(paymentmethod::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Paymentmethod, NewPaymentmethod, UpdatablePaymentmethod)(cfg)
    }


    // Fields of the GraphQL Paymentmethod type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Paymentmethod {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The payments of the paymentmethod, paginated
        async fn payments(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::payment::Payment>> {
            super::paymentmethod_payments::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the paymentmethod model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(paymentmethod::PaymentmethodQuery, ...);`
    #[derive(Default)]
    pub struct PaymentmethodQuery;

    #[Object]
    impl PaymentmethodQuery {
        async fn paymentmethod(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Paymentmethod> {
            find(id, app_state(ctx)?).await
        }

        async fn paymentmethods(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Paymentmethod>> {
            Ok(Paymentmethod::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the paymentmethod model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(paymentmethod::PaymentmethodMutation, ...);`
    #[derive(Default)]
    pub struct PaymentmethodMutation;

    #[Object]
    impl PaymentmethodMutation {
        async fn create_paymentmethod(&self, ctx: &Context<'_>, input: NewPaymentmethod) -> async_graphql::Result<Paymentmethod> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_paymentmethod(&self, ctx: &Context<'_>, input: UpdatablePaymentmethod) -> async_graphql::Result<Paymentmethod> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_paymentmethod(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Paymentmethod> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the paymentmethod up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Paymentmethod> {
        match Paymentmethod::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    