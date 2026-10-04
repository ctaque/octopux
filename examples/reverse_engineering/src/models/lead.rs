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
    #[octopux_info(path = "lead")]
    #[graphql(complex)]
    pub struct Lead {
        pub id: Id,
        pub email: String,
        pub name: String,
        pub campaign_id: Option<i64>,
        pub owner_id: Option<i64>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Lead")]
    pub struct NewLead {
        pub email: String,
        pub name: String,
        pub campaign_id: Option<i64>,
        pub owner_id: Option<i64>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Lead, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableLead {
        pub id: Id,
        pub email: String,
        pub name: String,
        pub campaign_id: Option<i64>,
        pub owner_id: Option<i64>,
    }

    // Registers the documented routes of the lead endpoint
    // (octopux `openapi` feature), to mount with `.configure(lead::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Lead, NewLead, UpdatableLead)(cfg)
    }


    // Fields of the GraphQL Lead type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Lead {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The opportunities of the lead, paginated
        async fn opportunities(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::opportunity::Opportunity>> {
            crate::lead_opportunities::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the lead model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(lead::LeadQuery, ...);`
    #[derive(Default)]
    pub struct LeadQuery;

    #[Object]
    impl LeadQuery {
        async fn lead(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Lead> {
            find(id, app_state(ctx)?).await
        }

        async fn leads(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Lead>> {
            Ok(Lead::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the lead model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(lead::LeadMutation, ...);`
    #[derive(Default)]
    pub struct LeadMutation;

    #[Object]
    impl LeadMutation {
        async fn create_lead(&self, ctx: &Context<'_>, input: NewLead) -> async_graphql::Result<Lead> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_lead(&self, ctx: &Context<'_>, input: UpdatableLead) -> async_graphql::Result<Lead> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_lead(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Lead> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the lead up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Lead> {
        match Lead::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    