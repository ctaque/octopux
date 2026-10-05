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
    use crate::invoice::{Invoice, Id};
    use crate::payment::Payment;
    use serde::Deserialize;
    use octopux::{
        HasMany,
        anyhow::Result,
        async_trait,
    };
    use apistos::ApiComponent;
    use schemars::JsonSchema;
    use octopux::gen_documented_relation_endpoint;

    #[derive(Deserialize, JsonSchema, ApiComponent)]
    pub struct InvoicePaymentsQuery {
        /// Number of rows to skip
        pub offset: Option<usize>,
        /// Maximum number of rows to return (20 by default, 100 at most)
        pub limit: Option<usize>,
    }
    /// Number of children returned when no limit is given
    const DEFAULT_LIMIT: i64 = 20;
    /// Maximum number of children returned
    const MAX_LIMIT: i64 = 100;

    /// The payments of a invoice, served on `GET /invoice/{id}/payments`
    pub struct InvoicePayments;

    #[async_trait]
    impl HasMany for InvoicePayments {
        type Parent = Invoice;
        type Id = Id;
        type Query = InvoicePaymentsQuery;
        type Result = Vec<Payment>;
        type State = AppState;
        const RELATION: &'static str = "payments";

        async fn list_related(id: Id, query: &InvoicePaymentsQuery, state: &AppState) -> Result<Option<Vec<Payment>>> {
            let offset = query.offset.unwrap_or(0) as i64;
            let limit = query.limit.map_or(DEFAULT_LIMIT, |l| (l as i64).min(MAX_LIMIT));
            let models = sqlx::query_as::<_, Payment>(
                "SELECT * FROM payment WHERE invoice_id = $1 ORDER BY id LIMIT $2 OFFSET $3",
            )
            .bind(id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.pool)
            .await?;
            // the parent is only looked up when the page is empty
            if models.is_empty() {
                let parent = sqlx::query_scalar::<_, Id>(
                    "SELECT id FROM invoice WHERE id = $1",
                )
                .bind(id)
                .fetch_optional(&state.pool)
                .await?;
                if parent.is_none() {
                    return Ok(None);
                }
            }
            Ok(Some(models))
        }
    }

    // Registers the route of the payments of a invoice, to mount with `.configure(invoice_payments::configure)`
    // in the same scope as the invoice routes
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_relation_endpoint!(InvoicePayments)(cfg)
    }

    // Resolves the `payments` field of the GraphQL Invoice type, declared in the `#[ComplexObject]` of its model
    pub async fn resolve(ctx: &async_graphql::Context<'_>, id: Id, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Payment>> {
        let state = ctx.data::<actix_web::web::Data<AppState>>()?;
        let children = InvoicePayments::list_related(id, &InvoicePaymentsQuery { offset, limit }, state.get_ref()).await?;
        // the parent was resolved, so it exists
        Ok(children.unwrap_or_default())
    }
    