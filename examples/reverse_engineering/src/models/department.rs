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
    #[octopux_info(path = "department")]
    #[graphql(complex)]
    pub struct Department {
        pub id: Id,
        pub name: String,
        pub company_id: i64,
        pub parent_id: Option<i64>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Department")]
    pub struct NewDepartment {
        pub name: String,
        pub company_id: i64,
        pub parent_id: Option<i64>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Department, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableDepartment {
        pub id: Id,
        pub name: String,
        pub company_id: i64,
        pub parent_id: Option<i64>,
    }

    // Registers the documented routes of the department endpoint
    // (octopux `openapi` feature), to mount with `.configure(department::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Department, NewDepartment, UpdatableDepartment)(cfg)
    }


    // Fields of the GraphQL Department type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Department {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The teams of the department, paginated
        async fn teams(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::team::Team>> {
            crate::department_teams::resolve(ctx, self.id, offset, limit).await
        }
        /// The employees of the department, paginated
        async fn employees(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::employee::Employee>> {
            crate::department_employees::resolve(ctx, self.id, offset, limit).await
        }
        /// The departments of the department, paginated
        async fn departments(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::department::Department>> {
            crate::department_departments::resolve(ctx, self.id, offset, limit).await
        }
        /// The budgets of the department, paginated
        async fn budgets(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::budget::Budget>> {
            crate::department_budgets::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the department model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(department::DepartmentQuery, ...);`
    #[derive(Default)]
    pub struct DepartmentQuery;

    #[Object]
    impl DepartmentQuery {
        async fn department(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Department> {
            find(id, app_state(ctx)?).await
        }

        async fn departments(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Department>> {
            Ok(Department::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the department model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(department::DepartmentMutation, ...);`
    #[derive(Default)]
    pub struct DepartmentMutation;

    #[Object]
    impl DepartmentMutation {
        async fn create_department(&self, ctx: &Context<'_>, input: NewDepartment) -> async_graphql::Result<Department> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_department(&self, ctx: &Context<'_>, input: UpdatableDepartment) -> async_graphql::Result<Department> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_department(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Department> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the department up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Department> {
        match Department::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    