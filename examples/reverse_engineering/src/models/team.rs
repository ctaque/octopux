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
    #[octopux_info(path = "team")]
    #[graphql(complex)]
    pub struct Team {
        pub id: Id,
        pub name: String,
        pub department_id: i64,
        pub lead_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Team")]
    pub struct NewTeam {
        pub name: String,
        pub department_id: i64,
        pub lead_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Team, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableTeam {
        pub id: Id,
        pub name: String,
        pub department_id: i64,
        pub lead_id: i64,
    }

    // Registers the documented routes of the team endpoint
    // (octopux `openapi` feature), to mount with `.configure(team::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Team, NewTeam, UpdatableTeam)(cfg)
    }


    // Fields of the GraphQL Team type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Team {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The employees of the team, paginated
        async fn employees(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::employee::Employee>> {
            super::team_employees::resolve(ctx, self.id, offset, limit).await
        }
        /// The teammembers of the team, paginated
        async fn teammembers(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::teammember::Teammember>> {
            super::team_teammembers::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the team model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(team::TeamQuery, ...);`
    #[derive(Default)]
    pub struct TeamQuery;

    #[Object]
    impl TeamQuery {
        async fn team(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Team> {
            find(id, app_state(ctx)?).await
        }

        async fn teams(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Team>> {
            Ok(Team::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the team model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(team::TeamMutation, ...);`
    #[derive(Default)]
    pub struct TeamMutation;

    #[Object]
    impl TeamMutation {
        async fn create_team(&self, ctx: &Context<'_>, input: NewTeam) -> async_graphql::Result<Team> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_team(&self, ctx: &Context<'_>, input: UpdatableTeam) -> async_graphql::Result<Team> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_team(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Team> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the team up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Team> {
        match Team::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    