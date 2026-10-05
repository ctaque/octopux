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
    #[octopux_info(path = "employee")]
    #[graphql(complex)]
    pub struct Employee {
        pub id: Id,
        pub matricule: String,
        pub first_name: String,
        pub last_name: String,
        pub account_id: i64,
        pub department_id: i64,
        pub job_title_id: i64,
        pub manager_id: Option<i64>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Employee")]
    pub struct NewEmployee {
        pub matricule: String,
        pub first_name: String,
        pub last_name: String,
        pub account_id: i64,
        pub department_id: i64,
        pub job_title_id: i64,
        pub manager_id: Option<i64>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Employee, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableEmployee {
        pub id: Id,
        pub matricule: String,
        pub first_name: String,
        pub last_name: String,
        pub account_id: i64,
        pub department_id: i64,
        pub job_title_id: i64,
        pub manager_id: Option<i64>,
    }

    // Registers the documented routes of the employee endpoint
    // (octopux `openapi` feature), to mount with `.configure(employee::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Employee, NewEmployee, UpdatableEmployee)(cfg)
    }


    // Fields of the GraphQL Employee type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Employee {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The timeentries of the employee, paginated
        async fn timeentries(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::timeentry::Timeentry>> {
            crate::employee_timeentries::resolve(ctx, self.id, offset, limit).await
        }
        /// The tickets of the employee, paginated
        async fn tickets(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::ticket::Ticket>> {
            crate::employee_tickets::resolve(ctx, self.id, offset, limit).await
        }
        /// The teams_by_employee of the employee, paginated
        async fn teams_by_employee(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::team::Team>> {
            crate::employee_teams_by_employee::resolve(ctx, self.id, offset, limit).await
        }
        /// The teammembers of the employee, paginated
        async fn teammembers(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::teammember::Teammember>> {
            crate::employee_teammembers::resolve(ctx, self.id, offset, limit).await
        }
        /// The teams of the employee, paginated
        async fn teams(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::team::Team>> {
            crate::employee_teams::resolve(ctx, self.id, offset, limit).await
        }
        /// The tasks of the employee, paginated
        async fn tasks(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::task::Task>> {
            crate::employee_tasks::resolve(ctx, self.id, offset, limit).await
        }
        /// The payslips of the employee, paginated
        async fn payslips(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::payslip::Payslip>> {
            crate::employee_payslips::resolve(ctx, self.id, offset, limit).await
        }
        /// The leaverequests of the employee, paginated
        async fn leaverequests(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::leaverequest::Leaverequest>> {
            crate::employee_leaverequests::resolve(ctx, self.id, offset, limit).await
        }
        /// The leads of the employee, paginated
        async fn leads(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::lead::Lead>> {
            crate::employee_leads::resolve(ctx, self.id, offset, limit).await
        }
        /// The expenses of the employee, paginated
        async fn expenses(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::expense::Expense>> {
            crate::employee_expenses::resolve(ctx, self.id, offset, limit).await
        }
        /// The employeeskills of the employee, paginated
        async fn employeeskills(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::employeeskill::Employeeskill>> {
            crate::employee_employeeskills::resolve(ctx, self.id, offset, limit).await
        }
        /// The employees of the employee, paginated
        async fn employees(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::employee::Employee>> {
            crate::employee_employees::resolve(ctx, self.id, offset, limit).await
        }
        /// The contracts of the employee, paginated
        async fn contracts(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::contract::Contract>> {
            crate::employee_contracts::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the employee model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(employee::EmployeeQuery, ...);`
    #[derive(Default)]
    pub struct EmployeeQuery;

    #[Object]
    impl EmployeeQuery {
        async fn employee(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Employee> {
            find(id, app_state(ctx)?).await
        }

        async fn employees(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Employee>> {
            Ok(Employee::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the employee model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(employee::EmployeeMutation, ...);`
    #[derive(Default)]
    pub struct EmployeeMutation;

    #[Object]
    impl EmployeeMutation {
        async fn create_employee(&self, ctx: &Context<'_>, input: NewEmployee) -> async_graphql::Result<Employee> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_employee(&self, ctx: &Context<'_>, input: UpdatableEmployee) -> async_graphql::Result<Employee> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_employee(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Employee> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the employee up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Employee> {
        match Employee::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    