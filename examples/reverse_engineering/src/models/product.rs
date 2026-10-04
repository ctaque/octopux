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
    #[octopux_info(path = "product")]
    #[graphql(complex)]
    pub struct Product {
        pub id: Id,
        pub sku: String,
        pub name: String,
        pub description: Option<String>,
        pub price: f64,
        pub brand_id: i64,
        pub category_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Product")]
    pub struct NewProduct {
        pub sku: String,
        pub name: String,
        pub description: Option<String>,
        pub price: f64,
        pub brand_id: i64,
        pub category_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Product, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableProduct {
        pub id: Id,
        pub sku: String,
        pub name: String,
        pub description: Option<String>,
        pub price: f64,
        pub brand_id: i64,
        pub category_id: i64,
    }

    // Registers the documented routes of the product endpoint
    // (octopux `openapi` feature), to mount with `.configure(product::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Product, NewProduct, UpdatableProduct)(cfg)
    }


    // Fields of the GraphQL Product type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Product {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The reviews of the product, paginated
        async fn reviews(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::review::Review>> {
            crate::product_reviews::resolve(ctx, self.id, offset, limit).await
        }
        /// The productvariants of the product, paginated
        async fn productvariants(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::productvariant::Productvariant>> {
            crate::product_productvariants::resolve(ctx, self.id, offset, limit).await
        }
        /// The tags of the product, paginated
        async fn tags(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::tag::Tag>> {
            crate::product_tags::resolve(ctx, self.id, offset, limit).await
        }
        /// The producttags of the product, paginated
        async fn producttags(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::producttag::Producttag>> {
            crate::product_producttags::resolve(ctx, self.id, offset, limit).await
        }
        /// The productimages of the product, paginated
        async fn productimages(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::productimage::Productimage>> {
            crate::product_productimages::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the product model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(product::ProductQuery, ...);`
    #[derive(Default)]
    pub struct ProductQuery;

    #[Object]
    impl ProductQuery {
        async fn product(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Product> {
            find(id, app_state(ctx)?).await
        }

        async fn products(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Product>> {
            Ok(Product::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the product model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(product::ProductMutation, ...);`
    #[derive(Default)]
    pub struct ProductMutation;

    #[Object]
    impl ProductMutation {
        async fn create_product(&self, ctx: &Context<'_>, input: NewProduct) -> async_graphql::Result<Product> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_product(&self, ctx: &Context<'_>, input: UpdatableProduct) -> async_graphql::Result<Product> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_product(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Product> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the product up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Product> {
        match Product::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    