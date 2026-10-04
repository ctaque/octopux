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
    #[octopux_info(path = "taskattachment")]
    #[graphql(complex)]
    pub struct Taskattachment {
        pub id: Id,
        pub task_id: i64,
        pub attachment_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Taskattachment")]
    pub struct NewTaskattachment {
        pub task_id: i64,
        pub attachment_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Taskattachment, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableTaskattachment {
        pub id: Id,
        pub task_id: i64,
        pub attachment_id: i64,
    }

    // Registers the documented routes of the taskattachment endpoint
    // (octopux `openapi` feature), to mount with `.configure(taskattachment::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Taskattachment, NewTaskattachment, UpdatableTaskattachment)(cfg)
    }


    // Fields of the GraphQL Taskattachment type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Taskattachment {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
    }

    // GraphQL queries of the taskattachment model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(taskattachment::TaskattachmentQuery, ...);`
    #[derive(Default)]
    pub struct TaskattachmentQuery;

    #[Object]
    impl TaskattachmentQuery {
        async fn taskattachment(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Taskattachment> {
            find(id, app_state(ctx)?).await
        }

        async fn taskattachments(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Taskattachment>> {
            Ok(Taskattachment::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the taskattachment model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(taskattachment::TaskattachmentMutation, ...);`
    #[derive(Default)]
    pub struct TaskattachmentMutation;

    #[Object]
    impl TaskattachmentMutation {
        async fn create_taskattachment(&self, ctx: &Context<'_>, input: NewTaskattachment) -> async_graphql::Result<Taskattachment> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_taskattachment(&self, ctx: &Context<'_>, input: UpdatableTaskattachment) -> async_graphql::Result<Taskattachment> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_taskattachment(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Taskattachment> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the taskattachment up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Taskattachment> {
        match Taskattachment::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    