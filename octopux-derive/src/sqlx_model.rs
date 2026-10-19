// `SqlxModel`, `SqlxNewModel` and `SqlxUpdatableModel`: the `Model`, `NewModel` and `UpdatableModel`
// implementations querying a table with sqlx, their SQL is written at compile time from the struct fields.
// The types of the queries and of the application state are read from the `http_*` attribute
// of the struct, the rest from `#[sqlx_model(...)]`.
use darling::FromMeta;
use proc_macro2::TokenStream;
use quote::quote;
use syn::ext::IdentExt;

use crate::{find_attr, parse_http_attr, HttpCreateDeriveParams, HttpFindListDeleteDeriveParams, HttpUpdateDeriveParams};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Database {
    Sqlite,
    Postgres,
    Mysql,
}

impl Database {
    pub(crate) fn parse(name: &str) -> Option<Database> {
        match name {
            "sqlite" => Some(Database::Sqlite),
            "postgres" => Some(Database::Postgres),
            "mysql" => Some(Database::Mysql),
            _ => None,
        }
    }

    // `$1, $2...` for SQLite and PostgreSQL, `?` for MySQL, numbered from `from`
    fn placeholders(self, from: usize, count: usize) -> Vec<String> {
        (from..from + count)
            .map(|i| if self == Database::Mysql { "?".to_string() } else { format!("${}", i) })
            .collect()
    }

    // The sqlx database type, `::octopux::__private::sqlx::Postgres`...
    pub(crate) fn sqlx_type(self) -> TokenStream {
        match self {
            Database::Sqlite => quote! { ::octopux::__private::sqlx::Sqlite },
            Database::Postgres => quote! { ::octopux::__private::sqlx::Postgres },
            Database::Mysql => quote! { ::octopux::__private::sqlx::MySql },
        }
    }

    // MySQL has no RETURNING
    fn returning(self) -> bool {
        self != Database::Mysql
    }
}

#[derive(Debug, FromMeta)]
struct SqlxModelArgs {
    /// `sqlite`, `postgres` or `mysql`
    database: String,
    /// Defaults to the lowercase name of the model
    #[darling(default)]
    table: Option<String>,
    /// The model returned by `save` (only for `SqlxNewModel`)
    #[darling(default)]
    model: Option<syn::Path>,
    /// Field of the application state holding the sqlx pool
    #[darling(default)]
    pool: Option<String>,
    /// The primary key column, `id` by default, a field of the model and of the updatable struct
    #[darling(default)]
    primary_key: Option<String>,
    /// The function giving the primary key of an inserted row, `uuid::Uuid::new_v4` (only for `SqlxNewModel`),
    /// the database generates it otherwise, unless the struct has a field of the primary key
    #[darling(default)]
    new_id: Option<syn::Path>,
    /// `created_at` and `updated_at` are set on insert, `updated_at` on update
    #[darling(default)]
    timestamps: bool,
    /// delete sets `deleted_at`, and find, list and update skip the deleted rows
    #[darling(default)]
    soft_delete: bool,
    /// `save` and `update` pass the payload through `BeforeSave::before_save` first
    #[darling(default)]
    before_save: bool,
    /// `list` pushes the conditions and the `ORDER BY` of the list query, which implements `SqlxFilter`
    #[darling(default)]
    filter: bool,
    #[darling(default)]
    default_limit: Option<i64>,
    #[darling(default)]
    max_limit: Option<i64>,
}

const DEFAULT_LIMIT: i64 = 20;
const DEFAULT_PRIMARY_KEY: &str = "id";
const MAX_LIMIT: i64 = 100;
const TIMESTAMP_COLUMNS: [&str; 3] = ["created_at", "updated_at", "deleted_at"];

struct Config {
    database: Database,
    table: String,
    pool: syn::Ident,
    primary_key: String,
    new_id: Option<syn::Path>,
    timestamps: bool,
    soft_delete: bool,
    before_save: bool,
    filter: bool,
    model: Option<syn::Path>,
    default_limit: i64,
    max_limit: i64,
}

// `default_table` gives the table when `table` is not set
fn parse_config(ast: &syn::DeriveInput, derive: &str, default_table: impl FnOnce(&SqlxModelArgs) -> Option<String>) -> syn::Result<Config> {
    let attr = find_attr(ast, "sqlx_model").ok_or_else(|| {
        syn::Error::new_spanned(&ast.ident, format!("{} requires the #[sqlx_model(database = \"...\")] attribute", derive))
    })?;
    let meta = &attr.meta;
    let args = SqlxModelArgs::from_meta(meta).map_err(|e| syn::Error::new_spanned(meta, e.to_string()))?;
    let database = Database::parse(&args.database).ok_or_else(|| {
        syn::Error::new_spanned(meta, format!("unknown database `{}`, use sqlite, postgres or mysql", args.database))
    })?;
    let table = match args.table.clone().or_else(|| default_table(&args)) {
        Some(table) => table,
        None => return Err(syn::Error::new_spanned(meta, format!("{} requires `table` or `model`", derive))),
    };
    let pool = syn::parse_str(args.pool.as_deref().unwrap_or("pool"))?;
    let primary_key = args.primary_key.clone().unwrap_or_else(|| DEFAULT_PRIMARY_KEY.to_string());
    if primary_key.is_empty() || !is_column_name(&primary_key) {
        return Err(syn::Error::new_spanned(
            meta,
            format!("`{}` is not a valid primary key column, use only latin letters (a-z, A-Z), digits and `_`", primary_key),
        ));
    }
    Ok(Config {
        database,
        table,
        pool,
        primary_key,
        new_id: args.new_id,
        timestamps: args.timestamps,
        soft_delete: args.soft_delete,
        before_save: args.before_save,
        filter: args.filter,
        model: args.model,
        default_limit: args.default_limit.unwrap_or(DEFAULT_LIMIT),
        max_limit: args.max_limit.unwrap_or(MAX_LIMIT),
    })
}

// Field identifiers of a struct with named fields, their columns are checked with `is_column_name`
fn named_fields<'a>(ast: &'a syn::DeriveInput, derive: &str) -> syn::Result<Vec<&'a syn::Ident>> {
    let fields: Vec<&syn::Ident> = match &ast.data {
        syn::Data::Struct(syn::DataStruct { fields: syn::Fields::Named(fields), .. }) => {
            fields.named.iter().filter_map(|f| f.ident.as_ref()).collect()
        }
        _ => return Err(syn::Error::new_spanned(&ast.ident, format!("{} only supports structs with named fields", derive))),
    };
    if let Some(invalid) = fields.iter().find(|f| !is_column_name(&column(f))) {
        return Err(syn::Error::new_spanned(
            invalid,
            format!("`{}` is not a valid column name, use only latin letters (a-z, A-Z), digits and `_`", column(invalid)),
        ));
    }
    Ok(fields)
}

// Rust identifiers accept any Unicode letter (`prénom`), columns are restricted to the latin alphabet
// so they need no quoting in the SQL and match in every database
pub(crate) fn is_column_name(name: &str) -> bool {
    name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

// Column of a field, `r#type` is the `type` column
pub(crate) fn column(field: &syn::Ident) -> String {
    field.unraw().to_string()
}

// The field of the primary key column
fn primary_key_field<'a>(ast: &syn::DeriveInput, fields: &[&'a syn::Ident], config: &Config, derive: &str) -> syn::Result<&'a syn::Ident> {
    fields.iter().copied().find(|f| column(f) == config.primary_key).ok_or_else(|| {
        syn::Error::new_spanned(&ast.ident, format!("{} requires an `{}` field, the primary key", derive, config.primary_key))
    })
}

// The `primary_key` of the `#[sqlx_model(...)]` attribute, None without it, the other arguments being left to the sqlx derives
pub(crate) fn primary_key(ast: &syn::DeriveInput) -> syn::Result<Option<String>> {
    let Some(attr) = find_attr(ast, "sqlx_model") else {
        return Ok(None);
    };
    let mut primary_key = None;
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("primary_key") {
            primary_key = Some(meta.value()?.parse::<syn::LitStr>()?.value());
        } else if meta.input.peek(syn::Token![=]) {
            meta.value()?.parse::<syn::Expr>()?;
        }
        Ok(())
    })?;
    Ok(primary_key)
}

fn not_deleted(config: &Config) -> &'static str {
    if config.soft_delete { " AND deleted_at IS NULL" } else { "" }
}

fn now() -> TokenStream {
    quote! { ::octopux::__private::chrono::Utc::now() }
}

// Binds `this`, the payload, transformed by `BeforeSave` with the `before_save` option
fn this(config: &Config, app_state: &syn::Ident) -> TokenStream {
    if config.before_save {
        quote! { let this = <Self as ::octopux::BeforeSave<#app_state>>::before_save(self, state).await?; }
    } else {
        quote! { let this = self; }
    }
}

pub fn impl_sqlx_model(ast: &syn::DeriveInput) -> syn::Result<TokenStream> {
    const DERIVE: &str = "SqlxModel";
    let HttpFindListDeleteDeriveParams(id_type, find_query, list_query, delete_query, app_state) =
        parse_http_attr(ast, "http_find_list_delete", DERIVE)?;
    let name = &ast.ident;
    let config = parse_config(ast, DERIVE, |_| Some(name.to_string().to_lowercase()))?;
    if let Some(new_id) = &config.new_id {
        return Err(syn::Error::new_spanned(new_id, "`new_id` is only read by SqlxNewModel"));
    }
    let fields = named_fields(ast, DERIVE)?;
    let id = primary_key_field(ast, &fields, &config, DERIVE)?;
    let db = config.database;
    let (table, pool, pk) = (&config.table, &config.pool, &config.primary_key);
    let p = db.placeholders(1, 3);

    let find_sql = format!("SELECT * FROM {} WHERE {} = {}{}", table, pk, p[0], not_deleted(&config));
    let list_sql = format!(
        "SELECT * FROM {}{} ORDER BY {} LIMIT {} OFFSET {}",
        table,
        if config.soft_delete { " WHERE deleted_at IS NULL" } else { "" },
        pk,
        p[0],
        p[1]
    );
    let select_by_id = format!("SELECT * FROM {} WHERE {} = {}", table, pk, p[0]);
    let now = now();
    let delete_body = match (config.soft_delete, db.returning()) {
        (true, true) => {
            let sql = format!("UPDATE {} SET deleted_at = {} WHERE {} = {}{} RETURNING *", table, p[0], pk, p[1], not_deleted(&config));
            quote! {
                let model = ::octopux::__private::sqlx::query_as::<_, #name>(#sql)
                    .bind(#now)
                    .bind(&self.#id)
                    .fetch_one(&state.#pool)
                    .await?;
            }
        }
        // the row is selected after its update, which fails when it was already deleted,
        // in the same transaction
        (true, false) => {
            let sql = format!("UPDATE {} SET deleted_at = {} WHERE {} = {}{}", table, p[0], pk, p[1], not_deleted(&config));
            quote! {
                let mut tx = state.#pool.begin().await?;
                let result = ::octopux::__private::sqlx::query(#sql)
                    .bind(#now)
                    .bind(&self.#id)
                    .execute(&mut *tx)
                    .await?;
                if result.rows_affected() == 0 {
                    return Err(::octopux::Error::NotFound.into());
                }
                let model = ::octopux::__private::sqlx::query_as::<_, #name>(#select_by_id)
                    .bind(&self.#id)
                    .fetch_one(&mut *tx)
                    .await?;
                tx.commit().await?;
            }
        }
        (false, true) => {
            let sql = format!("DELETE FROM {} WHERE {} = {} RETURNING *", table, pk, p[0]);
            quote! {
                let model = ::octopux::__private::sqlx::query_as::<_, #name>(#sql)
                    .bind(&self.#id)
                    .fetch_one(&state.#pool)
                    .await?;
            }
        }
        // the row is selected before its deletion, locked until the transaction commits
        (false, false) => {
            let sql = format!("DELETE FROM {} WHERE {} = {}", table, pk, p[0]);
            let select_for_update = format!("{} FOR UPDATE", select_by_id);
            quote! {
                let mut tx = state.#pool.begin().await?;
                let model = ::octopux::__private::sqlx::query_as::<_, #name>(#select_for_update)
                    .bind(&self.#id)
                    .fetch_one(&mut *tx)
                    .await?;
                ::octopux::__private::sqlx::query(#sql)
                    .bind(&self.#id)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
            }
        }
    };
    let (default_limit, max_limit) = (config.default_limit, config.max_limit);
    let list_body = if config.filter {
        // the query is built at runtime, its conditions and its order depending on the fields set
        let select = format!("SELECT * FROM {}{}", table, if config.soft_delete { " WHERE deleted_at IS NULL" } else { "" });
        let soft_delete = config.soft_delete;
        let sqlx_db = db.sqlx_type();
        let order_by = format!(" ORDER BY {}", pk);
        quote! {
            let mut qb = ::octopux::__private::sqlx::QueryBuilder::<#sqlx_db>::new(#select);
            let mut has_where = #soft_delete;
            ::octopux::SqlxFilter::<#sqlx_db>::push_filters(query, &mut qb, &mut has_where);
            if !::octopux::SqlxFilter::<#sqlx_db>::push_order_by(query, &mut qb)? {
                qb.push(#order_by);
            }
            qb.push(" LIMIT ").push_bind(limit).push(" OFFSET ").push_bind(offset);
            let models = qb.build_query_as::<#name>().fetch_all(&state.#pool).await?;
        }
    } else {
        quote! {
            let models = ::octopux::__private::sqlx::query_as::<_, #name>(#list_sql)
                .bind(limit)
                .bind(offset)
                .fetch_all(&state.#pool)
                .await?;
        }
    };
    Ok(quote! {
        #[::octopux::__private::async_trait]
        impl ::octopux::Model<#id_type, #find_query, #list_query, ::std::vec::Vec<#name>, #delete_query, #name, #app_state> for #name {
            async fn find(id: #id_type, _query: &#find_query, state: &#app_state) -> ::octopux::anyhow::Result<::std::boxed::Box<#name>> {
                let model = ::octopux::__private::sqlx::query_as::<_, #name>(#find_sql)
                    .bind(id)
                    .fetch_one(&state.#pool)
                    .await?;
                Ok(::std::boxed::Box::new(model))
            }
            async fn list(query: &#list_query, state: &#app_state) -> ::octopux::anyhow::Result<::std::vec::Vec<#name>> {
                // converted without `as`, which would wrap a huge value to a negative one: SQLite
                // reads a negative LIMIT as no limit at all
                let offset = <i64 as ::std::convert::TryFrom<_>>::try_from(query.offset.unwrap_or(0)).unwrap_or(i64::MAX).max(0);
                let limit = query.limit.map_or(#default_limit, |l| {
                    <i64 as ::std::convert::TryFrom<_>>::try_from(l).unwrap_or(i64::MAX).min(#max_limit).max(0)
                });
                #list_body
                Ok(models)
            }
            async fn delete(self: Self, _query: &#delete_query, state: &#app_state) -> ::octopux::anyhow::Result<#name> {
                #delete_body
                Ok(model)
            }
        }
    })
}

pub fn impl_sqlx_new_model(ast: &syn::DeriveInput) -> syn::Result<TokenStream> {
    const DERIVE: &str = "SqlxNewModel";
    let HttpCreateDeriveParams(save_query, app_state) = parse_http_attr(ast, "http_create", DERIVE)?;
    let name = &ast.ident;
    let config = parse_config(ast, DERIVE, |args| {
        args.model.as_ref().and_then(|m| m.segments.last()).map(|s| s.ident.to_string().to_lowercase())
    })?;
    let model = config.model.as_ref().ok_or_else(|| {
        syn::Error::new_spanned(name, "SqlxNewModel requires `model`, the model returned by `save`, in #[sqlx_model(...)]")
    })?;
    let fields = named_fields(ast, DERIVE)?;
    let db = config.database;
    let (table, pool, pk) = (&config.table, &config.pool, &config.primary_key);
    // the primary key given by the payload
    let payload_id = fields.iter().find(|f| column(f) == *pk);
    if let (Some(new_id), Some(_)) = (&config.new_id, payload_id) {
        return Err(syn::Error::new_spanned(new_id, format!("`new_id` gives the `{}` the struct already has", pk)));
    }

    let mut columns: Vec<String> = fields.iter().map(|f| column(f)).collect();
    let mut binds: Vec<TokenStream> = fields.iter().map(|f| quote! { .bind(&this.#f) }).collect();
    let new_id = match &config.new_id {
        Some(path) => {
            columns.insert(0, pk.clone());
            binds.insert(0, quote! { .bind(&new_id) });
            quote! { let new_id = #path(); }
        }
        None => quote! {},
    };
    // created_at and updated_at get the same value
    if config.timestamps {
        columns.extend(["created_at".to_string(), "updated_at".to_string()]);
        binds.extend([quote! { .bind(now) }, quote! { .bind(now) }]);
    }
    let insert = if columns.is_empty() {
        if db == Database::Mysql {
            format!("INSERT INTO {} () VALUES ()", table)
        } else {
            format!("INSERT INTO {} DEFAULT VALUES", table)
        }
    } else {
        format!(
            "INSERT INTO {} ({}) VALUES ({})",
            table,
            columns.join(", "),
            db.placeholders(1, columns.len()).join(", ")
        )
    };
    let now = if config.timestamps {
        let now = self::now();
        quote! { let now = #now; }
    } else {
        quote! {}
    };
    let body = if db.returning() {
        let sql = format!("{} RETURNING *", insert);
        quote! {
            let model = ::octopux::__private::sqlx::query_as::<_, #model>(#sql)
                #(#binds)*
                .fetch_one(&state.#pool)
                .await?;
        }
    } else {
        // the inserted row is selected in the same transaction, by the primary key given,
        // or else generated by the AUTO_INCREMENT of the column
        let select = format!("SELECT * FROM {} WHERE {} = ?", table, pk);
        let inserted_id = match (&config.new_id, payload_id) {
            (Some(_), _) => quote! { &new_id },
            (None, Some(f)) => quote! { &this.#f },
            (None, None) => quote! { result.last_insert_id() },
        };
        quote! {
            let mut tx = state.#pool.begin().await?;
            let result = ::octopux::__private::sqlx::query(#insert)
                #(#binds)*
                .execute(&mut *tx)
                .await?;
            let model = ::octopux::__private::sqlx::query_as::<_, #model>(#select)
                .bind(#inserted_id)
                .fetch_one(&mut *tx)
                .await?;
            tx.commit().await?;
        }
    };
    let this = this(&config, &app_state);
    Ok(quote! {
        #[::octopux::__private::async_trait]
        impl ::octopux::NewModel<#model, #save_query, #app_state> for #name {
            async fn save(self: Self, _query: &#save_query, state: &#app_state) -> ::octopux::anyhow::Result<#model> {
                #this
                #now
                #new_id
                #body
                Ok(model)
            }
        }
    })
}

pub fn impl_sqlx_updatable_model(ast: &syn::DeriveInput) -> syn::Result<TokenStream> {
    const DERIVE: &str = "SqlxUpdatableModel";
    let HttpUpdateDeriveParams(_id_type, update_query, output, _find_query, app_state) = parse_http_attr(ast, "http_update", DERIVE)?;
    let name = &ast.ident;
    let config = parse_config(ast, DERIVE, |_| Some(output.to_string().to_lowercase()))?;
    if let Some(new_id) = &config.new_id {
        return Err(syn::Error::new_spanned(new_id, "`new_id` is only read by SqlxNewModel"));
    }
    let fields = named_fields(ast, DERIVE)?;
    let id = primary_key_field(ast, &fields, &config, DERIVE)?;
    let db = config.database;
    let (table, pool, pk) = (&config.table, &config.pool, &config.primary_key);

    // with timestamps, the timestamp columns are not taken from the payload
    let set: Vec<&&syn::Ident> = fields
        .iter()
        .filter(|f| column(f) != *pk && !(config.timestamps && TIMESTAMP_COLUMNS.contains(&column(f).as_str())))
        .collect();
    let mut columns: Vec<String> = set.iter().map(|f| column(f)).collect();
    let mut binds: Vec<TokenStream> = set.iter().map(|f| quote! { .bind(&this.#f) }).collect();
    if config.timestamps {
        columns.push("updated_at".to_string());
        let now = now();
        binds.push(quote! { .bind(#now) });
    }
    if columns.is_empty() {
        return Err(syn::Error::new_spanned(name, format!("SqlxUpdatableModel requires a field to update besides `{}`", pk)));
    }
    let mut placeholders = db.placeholders(1, columns.len() + 1);
    let id_placeholder = placeholders.pop().unwrap_or_default();
    let assignments: Vec<String> = columns.iter().zip(&placeholders).map(|(c, p)| format!("{} = {}", c, p)).collect();
    let update = format!(
        "UPDATE {} SET {} WHERE {} = {}{}",
        table,
        assignments.join(", "),
        pk,
        id_placeholder,
        not_deleted(&config)
    );
    // the columns of the updatable struct
    let returned: Vec<String> = fields.iter().map(|f| column(f)).collect();
    let body = if db.returning() {
        let sql = format!("{} RETURNING {}", update, returned.join(", "));
        quote! {
            let model = ::octopux::__private::sqlx::query_as::<_, #name>(#sql)
                #(#binds)*
                .bind(&this.#id)
                .fetch_one(&state.#pool)
                .await?;
        }
    } else {
        // the updated row is selected in the same transaction, a missing row answering 404:
        // MySQL counts as affected only the rows whose values changed
        let select = format!("SELECT {} FROM {} WHERE {} = ?{}", returned.join(", "), table, pk, not_deleted(&config));
        quote! {
            let mut tx = state.#pool.begin().await?;
            ::octopux::__private::sqlx::query(#update)
                #(#binds)*
                .bind(&this.#id)
                .execute(&mut *tx)
                .await?;
            let model = ::octopux::__private::sqlx::query_as::<_, #name>(#select)
                .bind(&this.#id)
                .fetch_one(&mut *tx)
                .await?;
            tx.commit().await?;
        }
    };
    let this = this(&config, &app_state);
    Ok(quote! {
        #[::octopux::__private::async_trait]
        impl ::octopux::UpdatableModel<#name, #update_query, #app_state> for #name {
            async fn update(self: Self, _query: &#update_query, state: &#app_state) -> ::octopux::anyhow::Result<#name> {
                #this
                #body
                Ok(model)
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expand(f: fn(&syn::DeriveInput) -> syn::Result<TokenStream>, ast: syn::DeriveInput) -> String {
        f(&ast).map(|t| t.to_string()).unwrap_or_else(|e| format!("error: {}", e))
    }

    #[test]
    fn model_queries_the_lowercase_table() {
        let out = expand(impl_sqlx_model, syn::parse_quote! {
            #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
            #[sqlx_model(database = "postgres")]
            struct Project { id: Id, name: String }
        });
        assert!(out.contains("\"SELECT * FROM project WHERE id = $1\""), "{}", out);
        assert!(out.contains("\"SELECT * FROM project ORDER BY id LIMIT $1 OFFSET $2\""), "{}", out);
        assert!(out.contains("\"DELETE FROM project WHERE id = $1 RETURNING *\""), "{}", out);
    }

    #[test]
    fn soft_delete_skips_the_deleted_rows() {
        let out = expand(impl_sqlx_model, syn::parse_quote! {
            #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
            #[sqlx_model(database = "sqlite", table = "projects", soft_delete)]
            struct Project { id: Id, deleted_at: Option<DateTime<Utc>> }
        });
        assert!(out.contains("\"SELECT * FROM projects WHERE id = $1 AND deleted_at IS NULL\""), "{}", out);
        assert!(out.contains("\"SELECT * FROM projects WHERE deleted_at IS NULL ORDER BY id LIMIT $1 OFFSET $2\""), "{}", out);
        assert!(out.contains("\"UPDATE projects SET deleted_at = $1 WHERE id = $2 AND deleted_at IS NULL RETURNING *\""), "{}", out);
    }

    #[test]
    fn filter_lists_with_the_conditions_and_the_order_of_the_query() {
        let out = expand(impl_sqlx_model, syn::parse_quote! {
            #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
            #[sqlx_model(database = "postgres", soft_delete, filter)]
            struct Project { id: Id, deleted_at: Option<DateTime<Utc>> }
        });
        assert!(out.contains("QueryBuilder :: < :: octopux :: __private :: sqlx :: Postgres > :: new (\"SELECT * FROM project WHERE deleted_at IS NULL\")"), "{}", out);
        assert!(out.contains("let mut has_where = true"), "{}", out);
        assert!(out.contains(":: octopux :: SqlxFilter :: < :: octopux :: __private :: sqlx :: Postgres > :: push_filters (query , & mut qb , & mut has_where)"), "{}", out);
        assert!(out.contains("push_order_by (query , & mut qb) ?"), "{}", out);
        assert!(out.contains("qb . push (\" ORDER BY id\")"), "{}", out);
        assert!(!out.contains("LIMIT $1 OFFSET $2"), "{}", out);
        let without_soft_delete = expand(impl_sqlx_model, syn::parse_quote! {
            #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
            #[sqlx_model(database = "sqlite", filter)]
            struct Project { id: Id }
        });
        assert!(without_soft_delete.contains("new (\"SELECT * FROM project\")"), "{}", without_soft_delete);
        assert!(without_soft_delete.contains("let mut has_where = false"), "{}", without_soft_delete);
    }

    #[test]
    fn mysql_model_selects_instead_of_returning() {
        let out = expand(impl_sqlx_model, syn::parse_quote! {
            #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
            #[sqlx_model(database = "mysql", pool = "db")]
            struct Project { id: Id }
        });
        assert!(out.contains("\"SELECT * FROM project ORDER BY id LIMIT ? OFFSET ?\""), "{}", out);
        assert!(out.contains("\"DELETE FROM project WHERE id = ?\""), "{}", out);
        assert!(out.contains("\"SELECT * FROM project WHERE id = ? FOR UPDATE\""), "{}", out);
        assert!(out.contains("state . db . begin ()"), "{}", out);
        assert!(out.contains("tx . commit ()"), "{}", out);
        assert!(!out.contains("RETURNING"), "{}", out);
        assert!(out.contains("state . db"), "{}", out);
    }

    #[test]
    fn model_requires_an_id_and_its_attributes() {
        let no_id = expand(impl_sqlx_model, syn::parse_quote! {
            #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
            #[sqlx_model(database = "sqlite")]
            struct Project { name: String }
        });
        assert!(no_id.starts_with("error: SqlxModel requires an `id` field"), "{}", no_id);
        let no_http = expand(impl_sqlx_model, syn::parse_quote! {
            #[sqlx_model(database = "sqlite")]
            struct Project { id: Id }
        });
        assert!(no_http.contains("#[http_find_list_delete(...)]"), "{}", no_http);
        let unknown = expand(impl_sqlx_model, syn::parse_quote! {
            #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
            #[sqlx_model(database = "oracle")]
            struct Project { id: Id }
        });
        assert!(unknown.contains("unknown database `oracle`"), "{}", unknown);
    }

    #[test]
    fn new_model_inserts_its_fields_and_the_timestamps() {
        let out = expand(impl_sqlx_new_model, syn::parse_quote! {
            #[http_create(SaveQuery, AppState)]
            #[sqlx_model(database = "postgres", model = "Project", timestamps)]
            struct NewProject { name: String, r#type: i32 }
        });
        assert!(
            out.contains("\"INSERT INTO project (name, type, created_at, updated_at) VALUES ($1, $2, $3, $4) RETURNING *\""),
            "{}",
            out
        );
        assert!(out.contains("NewModel < Project , SaveQuery , AppState >"), "{}", out);
    }

    #[test]
    fn new_model_without_columns_inserts_the_defaults() {
        let sqlite = expand(impl_sqlx_new_model, syn::parse_quote! {
            #[http_create(SaveQuery, AppState)]
            #[sqlx_model(database = "sqlite", model = "Project")]
            struct NewProject {}
        });
        assert!(sqlite.contains("\"INSERT INTO project DEFAULT VALUES RETURNING *\""), "{}", sqlite);
        let mysql = expand(impl_sqlx_new_model, syn::parse_quote! {
            #[http_create(SaveQuery, AppState)]
            #[sqlx_model(database = "mysql", model = "Project")]
            struct NewProject {}
        });
        assert!(mysql.contains("\"INSERT INTO project () VALUES ()\""), "{}", mysql);
        assert!(mysql.contains("last_insert_id"), "{}", mysql);
        assert!(mysql.contains("execute (& mut * tx)"), "{}", mysql);
        assert!(mysql.contains("tx . commit ()"), "{}", mysql);
    }

    #[test]
    fn new_model_requires_its_model() {
        let out = expand(impl_sqlx_new_model, syn::parse_quote! {
            #[http_create(SaveQuery, AppState)]
            #[sqlx_model(database = "sqlite", table = "project")]
            struct NewProject { name: String }
        });
        assert!(out.starts_with("error: SqlxNewModel requires `model`"), "{}", out);
    }

    #[test]
    fn updatable_model_sets_its_fields_and_returns_them() {
        let out = expand(impl_sqlx_updatable_model, syn::parse_quote! {
            #[http_update(Id, UpdateQuery, Project, FindQuery, AppState)]
            #[sqlx_model(database = "postgres", timestamps, soft_delete)]
            struct UpdatableProject { id: Id, name: String, updated_at: Option<DateTime<Utc>> }
        });
        assert!(
            out.contains("\"UPDATE project SET name = $1, updated_at = $2 WHERE id = $3 AND deleted_at IS NULL RETURNING id, name, updated_at\""),
            "{}",
            out
        );
    }

    #[test]
    fn before_save_transforms_the_payload_first() {
        let new = expand(impl_sqlx_new_model, syn::parse_quote! {
            #[http_create(SaveQuery, AppState)]
            #[sqlx_model(database = "postgres", model = "User", before_save)]
            struct NewUser { password: String }
        });
        assert!(new.contains("let this = < Self as :: octopux :: BeforeSave < AppState >> :: before_save (self , state) . await ?"), "{}", new);
        let updatable = expand(impl_sqlx_updatable_model, syn::parse_quote! {
            #[http_update(Id, UpdateQuery, User, FindQuery, AppState)]
            #[sqlx_model(database = "postgres", before_save)]
            struct UpdatableUser { id: Id, password: String }
        });
        assert!(updatable.contains(":: before_save (self , state)"), "{}", updatable);
        let without = expand(impl_sqlx_new_model, syn::parse_quote! {
            #[http_create(SaveQuery, AppState)]
            #[sqlx_model(database = "postgres", model = "User")]
            struct NewUser { password: String }
        });
        assert!(!without.contains("BeforeSave"), "{}", without);
    }

    #[test]
    fn column_names_use_the_latin_alphabet() {
        let model = expand(impl_sqlx_model, syn::parse_quote! {
            #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
            #[sqlx_model(database = "sqlite")]
            struct Project { id: Id, prénom: String }
        });
        assert!(model.starts_with("error: `prénom` is not a valid column name"), "{}", model);
        let new = expand(impl_sqlx_new_model, syn::parse_quote! {
            #[http_create(SaveQuery, AppState)]
            #[sqlx_model(database = "sqlite", model = "Project")]
            struct NewProject { 名前: String }
        });
        assert!(new.starts_with("error: `名前` is not a valid column name"), "{}", new);
        let updatable = expand(impl_sqlx_updatable_model, syn::parse_quote! {
            #[http_update(Id, UpdateQuery, Project, FindQuery, AppState)]
            #[sqlx_model(database = "sqlite")]
            struct UpdatableProject { id: Id, line2_total: i32, r#type: i32 }
        });
        assert!(!updatable.starts_with("error"), "{}", updatable);
    }

    #[test]
    fn primary_key_replaces_the_id() {
        let model = expand(impl_sqlx_model, syn::parse_quote! {
            #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
            #[sqlx_model(database = "postgres", primary_key = "version", soft_delete)]
            struct Migration { version: Id, description: String }
        });
        assert!(model.contains("\"SELECT * FROM migration WHERE version = $1 AND deleted_at IS NULL\""), "{}", model);
        assert!(model.contains("\"SELECT * FROM migration WHERE deleted_at IS NULL ORDER BY version LIMIT $1 OFFSET $2\""), "{}", model);
        assert!(model.contains("\"UPDATE migration SET deleted_at = $1 WHERE version = $2 AND deleted_at IS NULL RETURNING *\""), "{}", model);
        assert!(model.contains(". bind (& self . version)"), "{}", model);
        let filter = expand(impl_sqlx_model, syn::parse_quote! {
            #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
            #[sqlx_model(database = "mysql", primary_key = "version", filter)]
            struct Migration { version: Id }
        });
        assert!(filter.contains("qb . push (\" ORDER BY version\")"), "{}", filter);
        assert!(filter.contains("\"SELECT * FROM migration WHERE version = ? FOR UPDATE\""), "{}", filter);
        let updatable = expand(impl_sqlx_updatable_model, syn::parse_quote! {
            #[http_update(Id, UpdateQuery, Migration, FindQuery, AppState)]
            #[sqlx_model(database = "mysql", primary_key = "version")]
            struct UpdatableMigration { version: Id, id: i32 }
        });
        assert!(updatable.contains("\"UPDATE migration SET id = ? WHERE version = ?\""), "{}", updatable);
        assert!(updatable.contains("\"SELECT version, id FROM migration WHERE version = ?\""), "{}", updatable);
        assert!(updatable.contains(". bind (& this . version)"), "{}", updatable);
    }

    #[test]
    fn primary_key_is_a_field_and_a_column() {
        let missing = expand(impl_sqlx_model, syn::parse_quote! {
            #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
            #[sqlx_model(database = "sqlite", primary_key = "version")]
            struct Migration { id: Id }
        });
        assert!(missing.starts_with("error: SqlxModel requires an `version` field"), "{}", missing);
        let invalid = expand(impl_sqlx_model, syn::parse_quote! {
            #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
            #[sqlx_model(database = "sqlite", primary_key = "id; DROP TABLE x")]
            struct Migration { id: Id }
        });
        assert!(invalid.contains("is not a valid primary key column"), "{}", invalid);
    }

    #[test]
    fn new_model_inserts_the_new_id() {
        let postgres = expand(impl_sqlx_new_model, syn::parse_quote! {
            #[http_create(SaveQuery, AppState)]
            #[sqlx_model(database = "postgres", model = "Project", new_id = "uuid::Uuid::new_v4")]
            struct NewProject { name: String }
        });
        assert!(postgres.contains("\"INSERT INTO project (id, name) VALUES ($1, $2) RETURNING *\""), "{}", postgres);
        assert!(postgres.contains("let new_id = uuid :: Uuid :: new_v4 () ;"), "{}", postgres);
        assert!(postgres.contains(". bind (& new_id) . bind (& this . name)"), "{}", postgres);
        let mysql = expand(impl_sqlx_new_model, syn::parse_quote! {
            #[http_create(SaveQuery, AppState)]
            #[sqlx_model(database = "mysql", model = "Project", primary_key = "uuid", new_id = "uuid::Uuid::new_v4")]
            struct NewProject { name: String }
        });
        assert!(mysql.contains("\"INSERT INTO project (uuid, name) VALUES (?, ?)\""), "{}", mysql);
        assert!(mysql.contains("\"SELECT * FROM project WHERE uuid = ?\""), "{}", mysql);
        assert!(mysql.contains(". bind (& new_id) . fetch_one"), "{}", mysql);
        assert!(!mysql.contains("last_insert_id"), "{}", mysql);
        let both = expand(impl_sqlx_new_model, syn::parse_quote! {
            #[http_create(SaveQuery, AppState)]
            #[sqlx_model(database = "mysql", model = "Project", new_id = "uuid::Uuid::new_v4")]
            struct NewProject { id: Id, name: String }
        });
        assert!(both.starts_with("error: `new_id` gives the `id` the struct already has"), "{}", both);
        let updatable = expand(impl_sqlx_updatable_model, syn::parse_quote! {
            #[http_update(Id, UpdateQuery, Project, FindQuery, AppState)]
            #[sqlx_model(database = "mysql", new_id = "uuid::Uuid::new_v4")]
            struct UpdatableProject { id: Id, name: String }
        });
        assert!(updatable.starts_with("error: `new_id` is only read by SqlxNewModel"), "{}", updatable);
    }

    #[test]
    fn mysql_new_model_selects_the_primary_key_of_the_payload() {
        let out = expand(impl_sqlx_new_model, syn::parse_quote! {
            #[http_create(SaveQuery, AppState)]
            #[sqlx_model(database = "mysql", model = "Migration", primary_key = "version")]
            struct NewMigration { version: i64, description: String }
        });
        assert!(out.contains("\"INSERT INTO migration (version, description) VALUES (?, ?)\""), "{}", out);
        assert!(out.contains("\"SELECT * FROM migration WHERE version = ?\""), "{}", out);
        assert!(out.contains(". bind (& this . version) . fetch_one"), "{}", out);
        assert!(!out.contains("last_insert_id"), "{}", out);
    }

    #[test]
    fn updatable_model_requires_a_column() {
        let out = expand(impl_sqlx_updatable_model, syn::parse_quote! {
            #[http_update(Id, UpdateQuery, Project, FindQuery, AppState)]
            #[sqlx_model(database = "sqlite")]
            struct UpdatableProject { id: Id }
        });
        assert!(out.starts_with("error: SqlxUpdatableModel requires a field to update"), "{}", out);
    }
}
