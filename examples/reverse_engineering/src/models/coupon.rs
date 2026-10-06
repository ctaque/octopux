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
    #[octopux_info(path = "coupon")]
    #[graphql(complex)]
    pub struct Coupon {
        pub id: Id,
        pub code: String,
        pub discount_percent: f64,
        pub active: bool,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Coupon")]
    pub struct NewCoupon {
        pub code: String,
        pub discount_percent: f64,
        pub active: bool,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Coupon, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableCoupon {
        pub id: Id,
        pub code: String,
        pub discount_percent: f64,
        pub active: bool,
    }

    // Registers the documented routes of the coupon endpoint
    // (octopux `openapi` feature), to mount with `.configure(coupon::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Coupon, NewCoupon, UpdatableCoupon)(cfg)
    }


    // Fields of the GraphQL Coupon type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Coupon {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The salesorders of the coupon, paginated
        async fn salesorders(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::salesorder::Salesorder>> {
            super::coupon_salesorders::resolve(ctx, self.id, offset, limit).await
        }
        /// The ordercoupons of the coupon, paginated
        async fn ordercoupons(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::ordercoupon::Ordercoupon>> {
            super::coupon_ordercoupons::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the coupon model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(coupon::CouponQuery, ...);`
    #[derive(Default)]
    pub struct CouponQuery;

    #[Object]
    impl CouponQuery {
        async fn coupon(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Coupon> {
            find(id, app_state(ctx)?).await
        }

        async fn coupons(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Coupon>> {
            Ok(Coupon::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the coupon model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(coupon::CouponMutation, ...);`
    #[derive(Default)]
    pub struct CouponMutation;

    #[Object]
    impl CouponMutation {
        async fn create_coupon(&self, ctx: &Context<'_>, input: NewCoupon) -> async_graphql::Result<Coupon> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_coupon(&self, ctx: &Context<'_>, input: UpdatableCoupon) -> async_graphql::Result<Coupon> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_coupon(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Coupon> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the coupon up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Coupon> {
        match Coupon::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    