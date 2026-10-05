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
    #[octopux_info(path = "productvariant")]
    #[graphql(complex)]
    pub struct Productvariant {
        pub id: Id,
        pub sku: String,
        pub name: String,
        pub price: f64,
        pub product_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Productvariant")]
    pub struct NewProductvariant {
        pub sku: String,
        pub name: String,
        pub price: f64,
        pub product_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Productvariant, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableProductvariant {
        pub id: Id,
        pub sku: String,
        pub name: String,
        pub price: f64,
        pub product_id: i64,
    }

    // Registers the documented routes of the productvariant endpoint
    // (octopux `openapi` feature), to mount with `.configure(productvariant::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Productvariant, NewProductvariant, UpdatableProductvariant)(cfg)
    }


    // Fields of the GraphQL Productvariant type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Productvariant {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The wishlists of the productvariant, paginated
        async fn wishlists(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::wishlist::Wishlist>> {
            crate::productvariant_wishlists::resolve(ctx, self.id, offset, limit).await
        }
        /// The wishlistitems of the productvariant, paginated
        async fn wishlistitems(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::wishlistitem::Wishlistitem>> {
            crate::productvariant_wishlistitems::resolve(ctx, self.id, offset, limit).await
        }
        /// The attributevalues of the productvariant, paginated
        async fn attributevalues(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::attributevalue::Attributevalue>> {
            crate::productvariant_attributevalues::resolve(ctx, self.id, offset, limit).await
        }
        /// The variantattributes of the productvariant, paginated
        async fn variantattributes(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::variantattribute::Variantattribute>> {
            crate::productvariant_variantattributes::resolve(ctx, self.id, offset, limit).await
        }
        /// The stockmovements of the productvariant, paginated
        async fn stockmovements(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::stockmovement::Stockmovement>> {
            crate::productvariant_stockmovements::resolve(ctx, self.id, offset, limit).await
        }
        /// The stocklevels of the productvariant, paginated
        async fn stocklevels(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::stocklevel::Stocklevel>> {
            crate::productvariant_stocklevels::resolve(ctx, self.id, offset, limit).await
        }
        /// The salesorderlines of the productvariant, paginated
        async fn salesorderlines(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::salesorderline::Salesorderline>> {
            crate::productvariant_salesorderlines::resolve(ctx, self.id, offset, limit).await
        }
        /// The quotelines of the productvariant, paginated
        async fn quotelines(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::quoteline::Quoteline>> {
            crate::productvariant_quotelines::resolve(ctx, self.id, offset, limit).await
        }
        /// The purchaseorderlines of the productvariant, paginated
        async fn purchaseorderlines(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::purchaseorderline::Purchaseorderline>> {
            crate::productvariant_purchaseorderlines::resolve(ctx, self.id, offset, limit).await
        }
        /// The pricelistitems of the productvariant, paginated
        async fn pricelistitems(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::pricelistitem::Pricelistitem>> {
            crate::productvariant_pricelistitems::resolve(ctx, self.id, offset, limit).await
        }
        /// The cartitems of the productvariant, paginated
        async fn cartitems(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::cartitem::Cartitem>> {
            crate::productvariant_cartitems::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the productvariant model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(productvariant::ProductvariantQuery, ...);`
    #[derive(Default)]
    pub struct ProductvariantQuery;

    #[Object]
    impl ProductvariantQuery {
        async fn productvariant(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Productvariant> {
            find(id, app_state(ctx)?).await
        }

        async fn productvariants(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Productvariant>> {
            Ok(Productvariant::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the productvariant model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(productvariant::ProductvariantMutation, ...);`
    #[derive(Default)]
    pub struct ProductvariantMutation;

    #[Object]
    impl ProductvariantMutation {
        async fn create_productvariant(&self, ctx: &Context<'_>, input: NewProductvariant) -> async_graphql::Result<Productvariant> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_productvariant(&self, ctx: &Context<'_>, input: UpdatableProductvariant) -> async_graphql::Result<Productvariant> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_productvariant(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Productvariant> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the productvariant up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Productvariant> {
        match Productvariant::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    