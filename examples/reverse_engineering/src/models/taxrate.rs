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
    #[octopux_info(path = "taxrate")]
    #[graphql(complex)]
    pub struct Taxrate {
        pub id: Id,
        pub name: String,
        pub rate: f64,
        pub country_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Taxrate")]
    pub struct NewTaxrate {
        pub name: String,
        pub rate: f64,
        pub country_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Taxrate, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableTaxrate {
        pub id: Id,
        pub name: String,
        pub rate: f64,
        pub country_id: i64,
    }

    // Registers the documented routes of the taxrate endpoint
    // (octopux `openapi` feature), to mount with `.configure(taxrate::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Taxrate, NewTaxrate, UpdatableTaxrate)(cfg)
    }


    // Fields of the GraphQL Taxrate type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Taxrate {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The salesorderlines of the taxrate, paginated
        async fn salesorderlines(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::salesorderline::Salesorderline>> {
            super::taxrate_salesorderlines::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the taxrate model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(taxrate::TaxrateQuery, ...);`
    #[derive(Default)]
    pub struct TaxrateQuery;

    #[Object]
    impl TaxrateQuery {
        async fn taxrate(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Taxrate> {
            find(id, app_state(ctx)?).await
        }

        async fn taxrates(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Taxrate>> {
            Ok(Taxrate::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the taxrate model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(taxrate::TaxrateMutation, ...);`
    #[derive(Default)]
    pub struct TaxrateMutation;

    #[Object]
    impl TaxrateMutation {
        async fn create_taxrate(&self, ctx: &Context<'_>, input: NewTaxrate) -> async_graphql::Result<Taxrate> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_taxrate(&self, ctx: &Context<'_>, input: UpdatableTaxrate) -> async_graphql::Result<Taxrate> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_taxrate(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Taxrate> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the taxrate up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Taxrate> {
        match Taxrate::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    