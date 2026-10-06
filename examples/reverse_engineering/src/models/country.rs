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
    #[octopux_info(path = "country")]
    #[graphql(complex)]
    pub struct Country {
        pub id: Id,
        pub code: String,
        pub name: String,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Country")]
    pub struct NewCountry {
        pub code: String,
        pub name: String,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Country, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableCountry {
        pub id: Id,
        pub code: String,
        pub name: String,
    }

    // Registers the documented routes of the country endpoint
    // (octopux `openapi` feature), to mount with `.configure(country::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Country, NewCountry, UpdatableCountry)(cfg)
    }


    // Fields of the GraphQL Country type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Country {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The taxrates of the country, paginated
        async fn taxrates(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::taxrate::Taxrate>> {
            super::country_taxrates::resolve(ctx, self.id, offset, limit).await
        }
        /// The suppliers of the country, paginated
        async fn suppliers(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::supplier::Supplier>> {
            super::country_suppliers::resolve(ctx, self.id, offset, limit).await
        }
        /// The regions of the country, paginated
        async fn regions(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::region::Region>> {
            super::country_regions::resolve(ctx, self.id, offset, limit).await
        }
        /// The companies of the country, paginated
        async fn companies(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::company::Company>> {
            super::country_companies::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the country model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(country::CountryQuery, ...);`
    #[derive(Default)]
    pub struct CountryQuery;

    #[Object]
    impl CountryQuery {
        async fn country(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Country> {
            find(id, app_state(ctx)?).await
        }

        async fn countries(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Country>> {
            Ok(Country::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the country model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(country::CountryMutation, ...);`
    #[derive(Default)]
    pub struct CountryMutation;

    #[Object]
    impl CountryMutation {
        async fn create_country(&self, ctx: &Context<'_>, input: NewCountry) -> async_graphql::Result<Country> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_country(&self, ctx: &Context<'_>, input: UpdatableCountry) -> async_graphql::Result<Country> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_country(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Country> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the country up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Country> {
        match Country::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    