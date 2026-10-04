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
    use chrono::{NaiveDate};
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
    #[octopux_info(path = "campaign")]
    #[graphql(complex)]
    pub struct Campaign {
        pub id: Id,
        pub name: String,
        pub budget: f64,
        pub start_date: NaiveDate,
        pub end_date: Option<NaiveDate>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Campaign")]
    pub struct NewCampaign {
        pub name: String,
        pub budget: f64,
        pub start_date: NaiveDate,
        pub end_date: Option<NaiveDate>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Campaign, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableCampaign {
        pub id: Id,
        pub name: String,
        pub budget: f64,
        pub start_date: NaiveDate,
        pub end_date: Option<NaiveDate>,
    }

    // Registers the documented routes of the campaign endpoint
    // (octopux `openapi` feature), to mount with `.configure(campaign::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Campaign, NewCampaign, UpdatableCampaign)(cfg)
    }


    // Fields of the GraphQL Campaign type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Campaign {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The leads of the campaign, paginated
        async fn leads(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::lead::Lead>> {
            crate::campaign_leads::resolve(ctx, self.id, offset, limit).await
        }
        /// The campaignchannels of the campaign, paginated
        async fn campaignchannels(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::campaignchannel::Campaignchannel>> {
            crate::campaign_campaignchannels::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the campaign model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(campaign::CampaignQuery, ...);`
    #[derive(Default)]
    pub struct CampaignQuery;

    #[Object]
    impl CampaignQuery {
        async fn campaign(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Campaign> {
            find(id, app_state(ctx)?).await
        }

        async fn campaigns(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Campaign>> {
            Ok(Campaign::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the campaign model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(campaign::CampaignMutation, ...);`
    #[derive(Default)]
    pub struct CampaignMutation;

    #[Object]
    impl CampaignMutation {
        async fn create_campaign(&self, ctx: &Context<'_>, input: NewCampaign) -> async_graphql::Result<Campaign> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_campaign(&self, ctx: &Context<'_>, input: UpdatableCampaign) -> async_graphql::Result<Campaign> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_campaign(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Campaign> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the campaign up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Campaign> {
        match Campaign::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    