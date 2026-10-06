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
    #[octopux_info(path = "company")]
    #[graphql(complex)]
    pub struct Company {
        pub id: Id,
        pub name: String,
        pub siret: String,
        pub country_id: i64,
        pub currency_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Company")]
    pub struct NewCompany {
        pub name: String,
        pub siret: String,
        pub country_id: i64,
        pub currency_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Company, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableCompany {
        pub id: Id,
        pub name: String,
        pub siret: String,
        pub country_id: i64,
        pub currency_id: i64,
    }

    // Registers the documented routes of the company endpoint
    // (octopux `openapi` feature), to mount with `.configure(company::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Company, NewCompany, UpdatableCompany)(cfg)
    }


    // Fields of the GraphQL Company type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Company {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The projects of the company, paginated
        async fn projects(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::project::Project>> {
            super::company_projects::resolve(ctx, self.id, offset, limit).await
        }
        /// The fiscalyears of the company, paginated
        async fn fiscalyears(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::fiscalyear::Fiscalyear>> {
            super::company_fiscalyears::resolve(ctx, self.id, offset, limit).await
        }
        /// The departments of the company, paginated
        async fn departments(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::department::Department>> {
            super::company_departments::resolve(ctx, self.id, offset, limit).await
        }
        /// The customers of the company, paginated
        async fn customers(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::customer::Customer>> {
            super::company_customers::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the company model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(company::CompanyQuery, ...);`
    #[derive(Default)]
    pub struct CompanyQuery;

    #[Object]
    impl CompanyQuery {
        async fn company(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Company> {
            find(id, app_state(ctx)?).await
        }

        async fn companies(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Company>> {
            Ok(Company::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the company model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(company::CompanyMutation, ...);`
    #[derive(Default)]
    pub struct CompanyMutation;

    #[Object]
    impl CompanyMutation {
        async fn create_company(&self, ctx: &Context<'_>, input: NewCompany) -> async_graphql::Result<Company> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_company(&self, ctx: &Context<'_>, input: UpdatableCompany) -> async_graphql::Result<Company> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_company(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Company> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the company up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Company> {
        match Company::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    