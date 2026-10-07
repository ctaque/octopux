extern crate proc_macro;
// The generated code only names items through `::octopux`, which re-exports these macros
// and the crates they rely on: the application needs no other import nor dependency.
use darling::FromMeta;
use quote::{quote, ToTokens};
use darling::ast::NestedMeta;
use syn::{ self, Result as SynResult, Token, parse_macro_input };

mod sqlx_filter;
mod sqlx_model;

struct HttpCreateDeriveParams (syn::Ident, syn::Ident);
impl syn::parse::Parse for HttpCreateDeriveParams {
    fn parse(input: syn::parse::ParseStream) -> SynResult<Self> {
        let content = input;
        let query = content.parse()?;
        content.parse::<Token![,]>()?;
        let app_state = content.parse()?;
        Ok(HttpCreateDeriveParams(query, app_state))
    }
}
#[proc_macro_derive(HttpCreate, attributes(http_create))]
pub fn http_create(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    impl_http_create_macro(&ast).unwrap_or_else(|e| e.to_compile_error()).into()
}

fn impl_http_create_macro(ast: &syn::DeriveInput) -> SynResult<proc_macro2::TokenStream> {
    let HttpCreateDeriveParams(query, app_state) = parse_http_attr(ast, "http_create", "HttpCreate")?;

    let name = &ast.ident;
    let gen = quote! {
        #[::octopux::__private::async_trait]
        impl ::octopux::HttpCreate<#query, #app_state> for #name {
            async fn http_create(payload: ::octopux::__private::actix_web::web::Json<Box<#name>>, query: ::octopux::__private::actix_web::web::Query<#query>, state: ::octopux::__private::actix_web::web::Data<#app_state>) -> ::octopux::__private::actix_web::HttpResponse{
                use ::octopux::NewModel as _;
                let params = query.into_inner();
                let to_save = payload.into_inner();
                let result = to_save.save(&params, &state).await;
                match result {
                    Ok(res) => ::octopux::__private::actix_web::HttpResponse::Ok().json(res),
                    Err(err) => ::octopux::__private::error_response(err)
                }
            }
        }
    };
    Ok(gen)
}

struct HttpFindListDeleteDeriveParams (syn::Ident, syn::Ident, syn::Ident, syn::Ident, syn::Ident);
impl syn::parse::Parse for HttpFindListDeleteDeriveParams {
    fn parse(input: syn::parse::ParseStream) -> SynResult<Self> {
        let content = input;
        let id = content.parse()?;
        content.parse::<Token![,]>()?;
        let find_query = content.parse()?;
        content.parse::<Token![,]>()?;
        let list_query = content.parse()?;
        content.parse::<Token![,]>()?;
        let delete_query = content.parse()?;
        content.parse::<Token![,]>()?;
        let app_state = content.parse()?;
        Ok(HttpFindListDeleteDeriveParams(id, find_query, list_query, delete_query, app_state))
    }
}


#[derive(Debug, FromMeta)]
struct RestfulInfo {
    pub path: String,
}

impl ToTokens for RestfulInfo {
    fn to_tokens(&self, _tokens: &mut proc_macro2::TokenStream) {
        ()
    }
}

#[proc_macro_attribute]
pub fn octopux_info(args: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let attrs_args = match NestedMeta::parse_meta_list(args.into()) {
        Ok(v) => v,
        Err(e) => { return proc_macro::TokenStream::from(darling::Error::from(e).write_errors()); }
    };
    let ast: syn::DeriveInput = match syn::parse(input.clone()) {
        Ok(ast) => ast,
        Err(e) => return e.to_compile_error().into(),
    };

    let args_tokens = match RestfulInfo::from_list(&attrs_args) {
        Ok(v) => v,
        Err(e) => { return proc_macro::TokenStream::from(e.write_errors()); }
    };
    let name  = ast.ident;
    let path =args_tokens.path;
    let gen = quote! {
        impl ::octopux::RestfulPathInfo for #name {
            fn path() -> String  {
                let p = #path;
                let p = p.to_string();
                p
            }
        }
    };
    let mut out:proc_macro::TokenStream = gen.into();
    out.extend::<proc_macro::TokenStream>(input);
    out
}

#[proc_macro_derive(HttpFindListDelete, attributes(http_find_list_delete))]
pub fn http_find_list_delete(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    impl_http_find_list_delete_macro(&ast).unwrap_or_else(|e| e.to_compile_error()).into()
}

fn impl_http_find_list_delete_macro(ast: &syn::DeriveInput) -> SynResult<proc_macro2::TokenStream> {
    let HttpFindListDeleteDeriveParams(id, find_query, list_query, delete_query, app_state) =
        parse_http_attr(ast, "http_find_list_delete", "HttpFindListDelete")?;

    let name = &ast.ident;
    let gen = quote! {
        #[derive(::octopux::__private::serde::Deserialize)]
        #[serde(crate = "::octopux::__private::serde")]
        #[doc(hidden)]
        pub struct OctopuxPath {
            id: #id
        }
        #[::octopux::__private::async_trait]
        impl ::octopux::HttpFindListDelete<OctopuxPath, #find_query, #list_query, #delete_query, #app_state> for #name {
            async fn http_list(
                query: ::octopux::__private::actix_web::web::Query<#list_query>,
                state: ::octopux::__private::actix_web::web::Data<#app_state>
            ) -> ::octopux::__private::actix_web::HttpResponse{
                use ::octopux::Model as _;
                let params = query.into_inner();
                let result = #name::list(&params, &state).await;
                match result {
                    Ok(res) => ::octopux::__private::actix_web::HttpResponse::Ok().json(res),
                    Err(err) => ::octopux::__private::error_response(err)
                }
            }
            async fn http_find(
                info: ::octopux::__private::actix_web::web::Path<OctopuxPath>,
                query: ::octopux::__private::actix_web::web::Query<#find_query>,
                state: ::octopux::__private::actix_web::web::Data<#app_state>
            ) -> ::octopux::__private::actix_web::HttpResponse {
                use ::octopux::Model as _;
                let params = query.into_inner();
                let result = #name::find(info.id.into(), &params, &state).await;
                match result {
                    Ok(res) => ::octopux::__private::actix_web::HttpResponse::Ok().json(res),
                    Err(err) => ::octopux::__private::error_response(err)
                }
            }
            async fn http_delete(
                info: ::octopux::__private::actix_web::web::Path<OctopuxPath>,
                query: ::octopux::__private::actix_web::web::Query<#delete_query>,
                state: ::octopux::__private::actix_web::web::Data<#app_state>
            ) -> ::octopux::__private::actix_web::HttpResponse {
                use ::octopux::Model as _;
                let params = query.into_inner();
                let find_params: #find_query = Default::default();
                let result = #name::find(info.id.into(), &find_params, &state).await;

                match result {
                    Ok(entity) => {
                        match entity.delete(&params, &state).await {
                            Ok(e) => ::octopux::__private::actix_web::HttpResponse::Ok().json(e),
                            Err(err) => ::octopux::__private::error_response(err)
                        }
                    }
                    Err(err) => ::octopux::__private::error_response(err)
                }
            }
        }
    };
    Ok(gen)
}

struct HttpUpdateDeriveParams (syn::Ident, syn::Ident, syn::Ident, syn::Ident, syn::Ident);
impl syn::parse::Parse for HttpUpdateDeriveParams {
    fn parse(input: syn::parse::ParseStream) -> SynResult<Self> {
        let content = input;
        let id = content.parse()?;
        content.parse::<syn::Token![,]>()?;
        let query = content.parse()?;
        content.parse::<syn::Token![,]>()?;
        let output = content.parse()?;
        content.parse::<syn::Token![,]>()?;
        let find_query = content.parse()?;
        content.parse::<Token![,]>()?;
        let app_state = content.parse()?;
        Ok(HttpUpdateDeriveParams(id, query, output, find_query, app_state))
    }
}

#[proc_macro_derive(HttpUpdate, attributes(http_update))]
pub fn http_update(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    impl_http_update_macro(&ast).unwrap_or_else(|e| e.to_compile_error()).into()
}

// The output and find query of the attribute are not used by the handler: `update` answers
// 404 itself when no entity has the id, `SqlxUpdatableModel` reading its table from the output.
fn impl_http_update_macro(ast: &syn::DeriveInput) -> SynResult<proc_macro2::TokenStream> {
    let HttpUpdateDeriveParams(id, query, _output, _find_query, app_state) =
        parse_http_attr(ast, "http_update", "HttpUpdate")?;

    let name = &ast.ident;
    // The path id identifies the entity to update: reject payloads targeting another one.
    let id_check = if has_named_field(ast, "id") {
        quote! {
            if to_update.id != info.id {
                return ::octopux::__private::actix_web::ResponseError::error_response(&::octopux::Error::IdMismatch);
            }
        }
    } else {
        quote! {}
    };
    let gen = quote! {
        #[derive(::octopux::__private::serde::Deserialize)]
        #[serde(crate = "::octopux::__private::serde")]
        #[doc(hidden)]
        pub struct OctopuxUpdatePath {
            id: #id
        }
        #[::octopux::__private::async_trait]
        impl ::octopux::HttpUpdate<OctopuxUpdatePath, #query, #app_state> for #name {
            async fn http_update(
                info: ::octopux::__private::actix_web::web::Path<OctopuxUpdatePath>,
                payload: ::octopux::__private::actix_web::web::Json<Box<#name>>,
                query: ::octopux::__private::actix_web::web::Query<#query>,
                state: ::octopux::__private::actix_web::web::Data<#app_state>
            ) -> ::octopux::__private::actix_web::HttpResponse {
                use ::octopux::{Model as _, UpdatableModel as _};
                let to_update = payload.into_inner();
                #id_check
                let params = query.into_inner();
                match to_update.update(&params, &state).await {
                    Ok(e) => ::octopux::__private::actix_web::HttpResponse::Ok().json(e),
                    Err(err) => ::octopux::__private::error_response(err)
                }
            }
        }
    };
    Ok(gen)
}

// The sqlx derives also declare the `http_*` attribute they read the types from,
// so that they can be used without the matching `Http*` derive.

/// Implements `Model` on the struct with sqlx queries on its table,
/// configured by `#[sqlx_model(database = "sqlite" | "postgres" | "mysql", ...)]`
#[proc_macro_derive(SqlxModel, attributes(sqlx_model, http_find_list_delete))]
pub fn sqlx_model(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    sqlx_model::impl_sqlx_model(&ast).unwrap_or_else(|e| e.to_compile_error()).into()
}

/// Implements `NewModel` on the struct with an sqlx INSERT of its fields
#[proc_macro_derive(SqlxNewModel, attributes(sqlx_model, http_create))]
pub fn sqlx_new_model(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    sqlx_model::impl_sqlx_new_model(&ast).unwrap_or_else(|e| e.to_compile_error()).into()
}

/// Implements `UpdatableModel` on the struct with an sqlx UPDATE of its fields
#[proc_macro_derive(SqlxUpdatableModel, attributes(sqlx_model, http_update))]
pub fn sqlx_updatable_model(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    sqlx_model::impl_sqlx_updatable_model(&ast).unwrap_or_else(|e| e.to_compile_error()).into()
}

/// Implements `SqlxFilter` on a list query, a condition on a column for each of its `Option` fields,
/// configured by `#[sqlx_filter(database = "sqlite" | "postgres" | "mysql")]`
#[proc_macro_derive(SqlxFilter, attributes(sqlx_filter))]
pub fn sqlx_filter(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    sqlx_filter::impl_sqlx_filter(&ast).unwrap_or_else(|e| e.to_compile_error()).into()
}

fn find_attr<'a>(ast: &'a syn::DeriveInput, name: &str) -> Option<&'a syn::Attribute> {
    ast.attrs.iter().find(|a| a.path().is_ident(name))
}

// The types of an `http_*` attribute, an error pointing at the struct when it is missing
// and at the attribute when it is invalid
fn parse_http_attr<T: syn::parse::Parse>(ast: &syn::DeriveInput, name: &str, derive: &str) -> SynResult<T> {
    let attr = find_attr(ast, name).ok_or_else(|| {
        syn::Error::new_spanned(&ast.ident, format!("{} requires the #[{}(...)] attribute", derive, name))
    })?;
    attr.parse_args()
}

fn has_named_field(ast: &syn::DeriveInput, field: &str) -> bool {
    match &ast.data {
        syn::Data::Struct(data) => data.fields.iter().any(|f| f.ident.as_ref().map_or(false, |i| i == field)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn idents(idents: &[&syn::Ident]) -> Vec<String> {
        idents.iter().map(|i| i.to_string()).collect()
    }

    // Parses `tokens` as written after the attribute name, `#[attr<tokens>]`, as the derives do
    fn params<T: syn::parse::Parse>(tokens: proc_macro2::TokenStream) -> SynResult<T> {
        let attr: syn::Attribute = syn::parse_quote! { #[attr #tokens] };
        attr.parse_args()
    }

    #[test]
    fn parses_http_create_params() {
        let HttpCreateDeriveParams(query, app_state) =
            params(quote! { (SaveQuery, AppState) }).unwrap();
        assert_eq!(idents(&[&query, &app_state]), ["SaveQuery", "AppState"]);
    }

    #[test]
    fn rejects_http_create_params_with_missing_argument() {
        assert!(params::<HttpCreateDeriveParams>(quote! { (SaveQuery) }).is_err());
    }

    #[test]
    fn rejects_http_create_params_with_extra_argument() {
        assert!(params::<HttpCreateDeriveParams>(quote! { (SaveQuery, AppState, Extra) }).is_err());
    }

    #[test]
    fn rejects_http_create_params_without_parentheses() {
        assert!(params::<HttpCreateDeriveParams>(quote! { = SaveQuery }).is_err());
    }

    #[test]
    fn parses_http_find_list_delete_params() {
        let HttpFindListDeleteDeriveParams(id, find_query, list_query, delete_query, app_state) =
            params(quote! { (Id, FindQuery, ListQuery, DeleteQuery, AppState) }).unwrap();
        assert_eq!(
            idents(&[&id, &find_query, &list_query, &delete_query, &app_state]),
            ["Id", "FindQuery", "ListQuery", "DeleteQuery", "AppState"]
        );
    }

    #[test]
    fn rejects_http_find_list_delete_params_with_missing_argument() {
        assert!(params::<HttpFindListDeleteDeriveParams>(quote! { (Id, FindQuery, ListQuery, DeleteQuery) }).is_err());
    }

    #[test]
    fn rejects_http_find_list_delete_params_with_wrong_separator() {
        assert!(params::<HttpFindListDeleteDeriveParams>(quote! { (Id; FindQuery; ListQuery; DeleteQuery; AppState) }).is_err());
    }

    #[test]
    fn parses_http_update_params() {
        let HttpUpdateDeriveParams(id, query, output, find_query, app_state) =
            params(quote! { (Id, UpdateQuery, Item, FindQuery, AppState) }).unwrap();
        assert_eq!(
            idents(&[&id, &query, &output, &find_query, &app_state]),
            ["Id", "UpdateQuery", "Item", "FindQuery", "AppState"]
        );
    }

    #[test]
    fn rejects_http_update_params_with_missing_argument() {
        assert!(params::<HttpUpdateDeriveParams>(quote! { (Id, UpdateQuery, Item, FindQuery) }).is_err());
    }

    #[test]
    fn detects_named_field() {
        let ast: syn::DeriveInput = syn::parse_quote! { struct UpdatableItem { id: i64, content: String } };
        assert!(has_named_field(&ast, "id"));
        assert!(!has_named_field(&ast, "uuid"));
    }

    #[test]
    fn ignores_fields_of_tuple_structs_and_enums() {
        let tuple: syn::DeriveInput = syn::parse_quote! { struct UpdatableItem(i64); };
        let enumeration: syn::DeriveInput = syn::parse_quote! { enum UpdatableItem { A { id: i64 } } };
        assert!(!has_named_field(&tuple, "id"));
        assert!(!has_named_field(&enumeration, "id"));
    }

    fn expand(f: fn(&syn::DeriveInput) -> SynResult<proc_macro2::TokenStream>, ast: syn::DeriveInput) -> String {
        f(&ast).map(|t| t.to_string()).unwrap_or_else(|e| format!("error: {}", e))
    }

    #[test]
    fn http_derives_report_a_missing_attribute() {
        let create = expand(impl_http_create_macro, syn::parse_quote! { struct NewItem { content: String } });
        assert_eq!(create, "error: HttpCreate requires the #[http_create(...)] attribute");
        let model = expand(impl_http_find_list_delete_macro, syn::parse_quote! { struct Item { id: Id } });
        assert_eq!(model, "error: HttpFindListDelete requires the #[http_find_list_delete(...)] attribute");
        let update = expand(impl_http_update_macro, syn::parse_quote! { struct UpdatableItem { id: Id } });
        assert_eq!(update, "error: HttpUpdate requires the #[http_update(...)] attribute");
    }

    #[test]
    fn http_derives_report_an_invalid_attribute() {
        let create = expand(impl_http_create_macro, syn::parse_quote! {
            #[http_create(SaveQuery)]
            struct NewItem { content: String }
        });
        assert!(create.starts_with("error: "), "{}", create);
        let update = expand(impl_http_update_macro, syn::parse_quote! {
            #[http_update(Id, UpdateQuery, Item, FindQuery)]
            struct UpdatableItem { id: Id }
        });
        assert!(update.starts_with("error: "), "{}", update);
    }

    #[test]
    fn http_update_does_not_find_the_entity_first() {
        let update = expand(impl_http_update_macro, syn::parse_quote! {
            #[http_update(Id, UpdateQuery, Item, FindQuery, AppState)]
            struct UpdatableItem { id: Id, content: String }
        });
        assert!(!update.contains("find"), "{}", update);
        assert!(update.contains("to_update . update (& params , & state)"), "{}", update);
    }

    fn restful_info(args: proc_macro2::TokenStream) -> darling::Result<RestfulInfo> {
        RestfulInfo::from_list(&NestedMeta::parse_meta_list(args).unwrap())
    }

    #[test]
    fn parses_restful_info() {
        let info = restful_info(quote! { path = "item" }).unwrap();
        assert_eq!(info.path, "item");
    }

    #[test]
    fn parses_restful_info_in_any_order() {
        let info = restful_info(quote! { path = "item" }).unwrap();
        assert_eq!(info.path, "item");
    }

    #[test]
    fn rejects_restful_info_without_path() {
        assert!(restful_info(quote! {}).is_err());
    }

    #[test]
    fn rejects_restful_info_with_unknown_field() {
        assert!(restful_info(quote! { path = "item", version = "2" }).is_err());
    }
}
