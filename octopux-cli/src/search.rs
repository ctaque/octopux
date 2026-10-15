// The search of a model by the meaning of its text, proposed for the `pgvector::Vector` fields of the models
// queried with sqlx on PostgreSQL: the vector is computed from the text fields of the model by an embedding
// model of fastembed when it is saved, and `GET /{model}/search?q=...` lists the entities nearest to the text.
// The code is generated in the project, octopux does not depend on fastembed: `src/embedder.rs`, once per
// project, only depends on fastembed, and `<table>_search.rs`, next to the model, converts its vectors
// to the `pgvector::Vector` of octopux.
use std::fs;
use std::io::{BufRead, Error, Write};
use std::path::Path;

use syn::spanned::Spanned;

use super::{
    bold, confirm, crate_module, cyan, declaration_hint, dim, extension_import_edit, failure, field_ident, highlight, line_indent, magenta,
    model_structs, parse_model_file, pluralize, prompt, source_path, success, to_camel_case, to_snake_case, warning, with_merged_root,
    write_source, Field, PGVECTOR, TIMESTAMP_COLUMNS,
};

// A text embedding model of fastembed, proposed for the searches
#[derive(Debug, PartialEq)]
pub(crate) struct EmbeddingModel {
    // the variant of `fastembed::EmbeddingModel`
    pub(crate) name: &'static str,
    pub(crate) dimensions: u32,
    what: &'static str,
    // the prefixes the model was trained with, before a search and before a text it should find
    query_prefix: &'static str,
    passage_prefix: &'static str,
    // the similarity of a search to the texts unrelated to it, measured on the recipes example (the base
    // models taken as their small model)
    unrelated: &'static str,
}

const BGE_QUERY_PREFIX: &str = "Represent this sentence for searching relevant passages: ";

// The models proposed, the first one by default
pub(crate) const EMBEDDING_MODELS: &[EmbeddingModel] = &[
    EmbeddingModel { name: "MultilingualE5Small", dimensions: 384, what: "multilingual, a search in French finds the texts in English", query_prefix: "query: ", passage_prefix: "passage: ", unrelated: "0.8" },
    EmbeddingModel { name: "MultilingualE5Base", dimensions: 768, what: "multilingual, more precise and slower", query_prefix: "query: ", passage_prefix: "passage: ", unrelated: "0.8" },
    EmbeddingModel { name: "BGESmallENV15", dimensions: 384, what: "English", query_prefix: BGE_QUERY_PREFIX, passage_prefix: "", unrelated: "0.55" },
    EmbeddingModel { name: "BGEBaseENV15", dimensions: 768, what: "English, more precise and slower", query_prefix: BGE_QUERY_PREFIX, passage_prefix: "", unrelated: "0.55" },
    EmbeddingModel { name: "AllMiniLML6V2", dimensions: 384, what: "English, the fastest", query_prefix: "", passage_prefix: "", unrelated: "0.3" },
];

// The search of a model
#[derive(Debug, PartialEq)]
pub(crate) struct Search {
    // the vector field computed from the text, `embedding`
    pub(crate) field: String,
    // the fields of the text, and whether they are optional
    pub(crate) text: Vec<(String, bool)>,
    pub(crate) model: &'static EmbeddingModel,
}

// Whether a field type is a dense vector of pgvector, which fastembed computes: `pgvector::Vector`,
// `Option<octopux::pgvector::Vector>`
pub(crate) fn is_dense_vector(ty: &str) -> bool {
    let ty: String = ty.chars().filter(|c| !c.is_whitespace()).collect();
    let ty = ty.strip_prefix("Option<").and_then(|t| t.strip_suffix('>')).unwrap_or(&ty);
    matches!(ty.trim_start_matches("octopux::"), "pgvector::Vector")
}

// The vector type of an optional vector field: `Option<pgvector::Vector>` gives `pgvector::Vector`
pub(crate) fn vector_type(ty: &str) -> String {
    let ty: String = ty.chars().filter(|c| !c.is_whitespace()).collect();
    ty.strip_prefix("Option<").and_then(|t| t.strip_suffix('>')).map_or(ty.clone(), String::from)
}

// The text field of a field type, Some(true) for an `Option<String>`, Some(false) for a `String`
pub(crate) fn text_type(ty: &str) -> Option<bool> {
    let ty: String = ty.chars().filter(|c| !c.is_whitespace()).collect();
    match ty.as_str() {
        "String" | "std::string::String" => Some(false),
        "Option<String>" | "Option<std::string::String>" => Some(true),
        _ => None,
    }
}

// The fastembed model of an existing src/embedder.rs, None when it names none of `EMBEDDING_MODELS`
pub(crate) fn embedder_model(source: &str) -> Option<&'static EmbeddingModel> {
    source.match_indices("EmbeddingModel::").find_map(|(at, prefix)| {
        let rest = &source[at + prefix.len()..];
        let name = &rest[..rest.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(rest.len())];
        EMBEDDING_MODELS.iter().find(|m| m.name == name)
    })
}

// Asks whether the first vector field of `fields` the user picks is computed from the text fields `texts`,
// and with which model, the one of `embedder` (an existing src/embedder.rs) when set.
// None when no vector is picked, or when the model has no text field
pub(crate) fn read_search<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    fields: &[Field],
    texts: &[(String, bool)],
    embedder: Option<&'static EmbeddingModel>,
) -> Result<Option<Search>, Error> {
    let vectors: Vec<&Field> = fields.iter().filter(|f| is_dense_vector(&f.ty)).collect();
    if vectors.is_empty() {
        return Ok(None);
    }
    writeln!(output, "\n{}", bold("Search"))?;
    let mut picked = None;
    for vector in vectors {
        let message = format!("`{}` is a vector: compute it from the text of the model with fastembed, and search the model by meaning?", vector.name);
        if confirm(input, output, &message)? {
            picked = Some(vector.name.clone());
            break;
        }
    }
    let Some(field) = picked else { return Ok(None) };
    if texts.is_empty() {
        writeln!(output, "{}", warning(&format!("The model has no `String` field to compute `{}` from, search not generated", field)))?;
        return Ok(None);
    }
    let model = match embedder {
        Some(model) => {
            writeln!(output, "  {}", dim(&format!("src/embedder.rs computes the vectors with {} ({} dimensions)", model.name, model.dimensions)))?;
            model
        }
        None => read_embedding_model(input, output)?,
    };
    let text = read_text_fields(input, output, &field, texts)?;
    Ok(Some(Search { field, text, model }))
}

fn read_embedding_model<R: BufRead, W: Write>(input: &mut R, output: &mut W) -> Result<&'static EmbeddingModel, Error> {
    for (i, model) in EMBEDDING_MODELS.iter().enumerate() {
        writeln!(output, "  {} {} {}", magenta(&format!("{})", i + 1)), model.name, dim(&format!("{} dimensions, {}", model.dimensions, model.what)))?;
    }
    loop {
        let message = format!("{} {} {} ", cyan("?"), bold("Embedding model ›"), dim(&format!("[{}]", EMBEDDING_MODELS[0].name)));
        let answer = prompt(input, output, &message)?.unwrap_or_default();
        if answer.is_empty() {
            return Ok(&EMBEDDING_MODELS[0]);
        }
        let chosen = answer
            .parse::<usize>()
            .ok()
            .and_then(|n| EMBEDDING_MODELS.get(n.wrapping_sub(1)))
            .or_else(|| EMBEDDING_MODELS.iter().find(|m| m.name.eq_ignore_ascii_case(&answer)));
        match chosen {
            Some(model) => return Ok(model),
            None => writeln!(output, "{}", failure(&format!("`{}` is not a model, enter a number between 1 and {}", answer, EMBEDDING_MODELS.len())))?,
        }
    }
}

// Asks for the fields of the text, separated by commas, all of them by default
fn read_text_fields<R: BufRead, W: Write>(input: &mut R, output: &mut W, field: &str, texts: &[(String, bool)]) -> Result<Vec<(String, bool)>, Error> {
    let names: Vec<&str> = texts.iter().map(|(name, _)| name.as_str()).collect();
    loop {
        let message = format!(
            "{} {} {} ",
            cyan("?"),
            bold(&format!("Fields {} is computed from ›", cyan(&format!("`{}`", field)))),
            dim(&format!("(separated by commas) [{}]", names.join(", ")))
        );
        let answer = prompt(input, output, &message)?.unwrap_or_default();
        if answer.is_empty() {
            return Ok(texts.to_vec());
        }
        let mut picked = Vec::new();
        let mut unknown = Vec::new();
        for name in answer.split(',').map(str::trim).filter(|n| !n.is_empty()) {
            match texts.iter().find(|(text, _)| text == name) {
                Some(text) if !picked.contains(text) => picked.push(text.clone()),
                Some(_) => {}
                None => unknown.push(name.to_string()),
            }
        }
        match (unknown.is_empty(), picked.is_empty()) {
            (true, false) => return Ok(picked),
            (false, _) => writeln!(output, "{}", failure(&format!("`{}` is not a text field, pick among {}", unknown.join("`, `"), names.join(", "))))?,
            (true, true) => writeln!(output, "{}", failure(&format!("Pick at least one field among {}", names.join(", "))))?,
        }
    }
}

// The vector field of the search: optional, so that the rows saved before the search are kept,
// of the dimensions of the model
pub(crate) fn search_field(field: &Field, model: &EmbeddingModel) -> Field {
    let ty: String = field.ty.chars().filter(|c| !c.is_whitespace()).collect();
    let ty = if ty.starts_with("Option<") { ty } else { format!("Option<{}>", ty) };
    Field { ty, length: Some(model.dimensions), unique: false, references: None, ..field.clone() }
}

// The text fields of the model struct of a model file
pub(crate) fn model_text_fields(source: &str, model: &str) -> Result<Vec<(String, bool)>, String> {
    let file = parse_model_file(source)?;
    let structs = model_structs(&file, model)?;
    Ok(structs[0]
        .1
        .named
        .iter()
        .filter_map(|f| {
            let name = syn::ext::IdentExt::unraw(f.ident.as_ref()?).to_string();
            text_type(&source[f.ty.span().byte_range()]).map(|optional| (name, optional))
        })
        .collect())
}

// Whether the model already transforms its payloads with `before_save`, which a search would implement again
pub(crate) fn has_before_save(source: &str, model: &str) -> bool {
    let Ok(file) = parse_model_file(source) else { return false };
    let Ok(structs) = model_structs(&file, model) else { return false };
    structs[1..].iter().any(|(item, _)| sqlx_model_attribute(source, item).is_some_and(|attr| has_word(attr, "before_save")))
}

pub(crate) fn has_word(text: &str, word: &str) -> bool {
    text.split(|c: char| !c.is_alphanumeric() && c != '_').any(|w| w == word)
}

// The text of the `#[sqlx_model(...)]` attribute of a struct
fn sqlx_model_attribute<'a>(source: &'a str, item: &syn::ItemStruct) -> Option<&'a str> {
    item.attrs.iter().map(|attr| &source[attr.span().byte_range()]).find(|text| text.starts_with("#[sqlx_model"))
}

// The edits inserting the declarations `lines` in the struct `named`, before its timestamps, last otherwise
pub(crate) fn insert_fields_edits(source: &str, named: &syn::FieldsNamed, lines: &[String]) -> Vec<(usize, String)> {
    let declaration = |indent: &str| lines.iter().map(|line| format!("{}{}\n", indent, line)).collect::<String>();
    let inline = || lines.iter().map(|line| format!("{} ", line)).collect::<String>();
    let mut edits = Vec::new();
    let timestamp = named.named.iter().find(|f| f.ident.as_ref().is_some_and(|i| TIMESTAMP_COLUMNS.iter().any(|c| i == c)));
    match timestamp {
        Some(field) => {
            // before the attributes and the documentation of the timestamp
            let pos = field.attrs.first().map_or_else(|| field.span().byte_range().start, |a| a.span().byte_range().start);
            match line_indent(source, pos) {
                (start, Some(indent)) => edits.push((start, declaration(indent))),
                (_, None) => edits.push((pos, inline())),
            }
        }
        None => {
            let close = named.brace_token.span.close().byte_range().start;
            if let Some(last) = named.named.last().filter(|_| !named.named.trailing_punct()) {
                edits.push((last.span().byte_range().end, ",".to_string()));
            }
            match line_indent(source, close) {
                (start, Some(indent)) => {
                    let indent = match named.named.last().map(|f| line_indent(source, f.span().byte_range().start).1) {
                        Some(Some(field_indent)) => field_indent.to_string(),
                        _ => format!("{}    ", indent),
                    };
                    edits.push((start, declaration(&indent)));
                }
                (_, None) => edits.push((close, format!(" {}", inline()))),
            }
        }
    }
    edits
}

pub(crate) fn apply_edits(source: &str, mut edits: Vec<(usize, String)>) -> String {
    // from the end, so that the offsets of the other edits stay valid
    edits.sort_by_key(|(pos, _)| std::cmp::Reverse(*pos));
    let mut patched = source.to_string();
    for (pos, text) in edits {
        patched.insert_str(pos, &text);
    }
    patched
}

// `source` with the vector field of the search in the creatable and updatable structs of `model`, skipped by serde
// and GraphQL: `before_save` computes it, the clients never send it. The model struct does not declare it,
// the entities are sent without their vector
pub(crate) fn with_search_field(source: &str, model: &str, field: &Field) -> Result<String, String> {
    let file = parse_model_file(source)?;
    let graphql = source.contains("InputObject");
    let mut lines = vec!["/// Computed from the text by `before_save`, never read from the payload".to_string(), "#[serde(skip)]".to_string()];
    if graphql {
        lines.push("#[graphql(skip)]".to_string());
    }
    lines.push(format!("pub {}: {},", field_ident(&field.name), field.ty));
    let mut edits = Vec::new();
    for (_, named) in &model_structs(&file, model)?[1..] {
        edits.extend(insert_fields_edits(source, named, &lines));
    }
    edits.extend(extension_import_edit(source, &file, std::slice::from_ref(field), &PGVECTOR));
    Ok(apply_edits(source, edits))
}

// `source` with the model of `model` set up for the search: its list query filtered by `SqlxFilter` with the
// `near` vector, nearest first, and the payloads of its creatable and updatable structs passed through `before_save`
pub(crate) fn with_search(source: &str, model: &str, search: &Search, vector_type: &str) -> Result<String, String> {
    let file = parse_model_file(source)?;
    let structs = model_structs(&file, model)?;
    let mut edits = Vec::new();
    for ((item, _), option) in structs.iter().zip(["filter", "before_save", "before_save"]) {
        let attr = item.attrs.iter().find(|a| source[a.span().byte_range()].starts_with("#[sqlx_model"));
        let Some(attr) = attr else { return Err(format!("`{}` has no `#[sqlx_model(...)]`, the search needs the sqlx derives", item.ident)) };
        let range = attr.span().byte_range();
        if !has_word(&source[range.clone()], option) {
            // before the `)]` closing the attribute
            edits.push((range.end - 2, format!(", {}", option)));
        }
    }
    let list = file.items.iter().find_map(|item| match item {
        syn::Item::Struct(s) if s.ident == "ListQuery" => Some(s),
        _ => None,
    });
    let Some(list) = list else { return Err("no `ListQuery` struct".to_string()) };
    let syn::Fields::Named(list_fields) = &list.fields else { return Err("`ListQuery` is not a struct with named fields".to_string()) };
    if list_fields.named.iter().any(|f| f.ident.as_ref().is_some_and(|i| i == "near")) {
        return Err("`ListQuery` already declares a `near` field".to_string());
    }
    let derive = list.attrs.iter().find(|a| source[a.span().byte_range()].starts_with("#[derive"));
    let Some(derive) = derive else { return Err("`ListQuery` has no derive".to_string()) };
    let range = derive.span().byte_range();
    let text = &source[range.clone()];
    if !has_word(text, "Default") {
        // `ListQuery { near, ..Default::default() }` in the search
        edits.push((range.start + text.find('(').map_or(0, |i| i + 1), "Default, ".to_string()));
    }
    if !has_word(text, "SqlxFilter") {
        edits.push((range.end - 2, ", SqlxFilter".to_string()));
    }
    if !list.attrs.iter().any(|a| source[a.span().byte_range()].starts_with("#[sqlx_filter")) {
        let indent = line_indent(source, range.start).1.unwrap_or_default();
        edits.push((range.end, format!("\n{}#[sqlx_filter(database = \"postgres\")]", indent)));
    }
    let mut near = vec![
        "/// The rows nearest to this vector, by cosine distance: `ORDER BY ... <=> ...`. Set by the search only,".to_string(),
        "/// from the vector of its text: never read from the query string".to_string(),
        format!("#[sqlx_filter(column = \"{}\", op = \"nearest\")]", search.field),
        "#[serde(skip)]".to_string(),
    ];
    if source.contains("JsonSchema") {
        near.push("#[schemars(skip)]".to_string());
    }
    near.push(format!("pub near: Option<{}>,", vector_type));
    edits.extend(insert_fields_edits(source, list_fields, &near));
    edits.extend(sqlx_filter_import_edit(source, &file));
    // the list resolver of --graphql leaves `near` unset
    Ok(apply_edits(source, edits)
        .replace("ListQuery { offset, limit }", "ListQuery { offset, limit, ..Default::default() }")
        .replace("ListQuery { offset, limit, sort, order }", "ListQuery { offset, limit, sort, order, ..Default::default() }"))
}

// The edit importing `SqlxFilter` in the `use octopux::{...};` of the file, or in a new `use` after the last one
fn sqlx_filter_import_edit(source: &str, file: &syn::File) -> Option<(usize, String)> {
    let uses: Vec<&syn::ItemUse> = file.items.iter().filter_map(|item| match item {
        syn::Item::Use(u) => Some(u),
        _ => None,
    }).collect();
    let group = uses.iter().find_map(|u| match &u.tree {
        syn::UseTree::Path(path) if path.ident == "octopux" => match path.tree.as_ref() {
            syn::UseTree::Group(group) => Some(group),
            _ => None,
        },
        _ => None,
    });
    if let Some(group) = group {
        if group.items.iter().any(|t| matches!(t, syn::UseTree::Name(n) if n.ident == "SqlxFilter")) {
            return None;
        }
        let close = group.brace_token.span.close().byte_range().start;
        return match (group.items.last(), line_indent(source, close)) {
            // one import per line
            (Some(last), (start, Some(_))) if line_indent(source, last.span().byte_range().start).1.is_some() => {
                let indent = line_indent(source, last.span().byte_range().start).1.unwrap_or_default();
                if group.items.trailing_punct() {
                    Some((start, format!("{}SqlxFilter,\n", indent)))
                } else {
                    Some((last.span().byte_range().end, format!(",\n{}SqlxFilter", indent)))
                }
            }
            (None, _) => Some((close, "SqlxFilter".to_string())),
            _ if group.items.trailing_punct() => Some((close, " SqlxFilter".to_string())),
            _ => Some((close, ", SqlxFilter".to_string())),
        };
    }
    let last = uses.last()?;
    let indent = line_indent(source, last.span().byte_range().start).1.unwrap_or_default();
    Some((last.semi_token.span().byte_range().end, format!("\n{}use octopux::SqlxFilter;", indent)))
}

const EMBEDDER_TPL: &str = r#"
// The embedding model of the searches, run locally on the CPU by fastembed, downloaded from Hugging Face on
// the first run into `.fastembed_cache`, then loaded from it. Generated once by `octopux` for the searches of
// all the models, to edit freely: another model, a GPU, the cache folder...
// The models of the searches convert the vectors to `octopux::pgvector::Vector`
use std::sync::{Arc, Mutex};

use fastembed::{EmbeddingModel, Error, Result, TextEmbedding, TextInitOptions};

/// {model}, {what}: its vectors have {dimensions} dimensions, those of the `vector({dimensions})` columns
/// of the searches. Another model needs columns of its dimensions, and the vectors computed again
pub const MODEL: EmbeddingModel = EmbeddingModel::{model};
/// The prefixes the model was trained with, before a search and before a text it should find
const QUERY_PREFIX: &str = "{query_prefix}";
const PASSAGE_PREFIX: &str = "{passage_prefix}";
/// Texts embedded at once
pub const BATCH_SIZE: usize = 64;

/// `TextEmbedding::embed` takes `&mut self`, so the workers share the model behind a mutex,
/// and it runs on the blocking threads of the runtime
#[derive(Clone)]
pub struct Embedder(Arc<Mutex<TextEmbedding>>);

impl Embedder {
    /// Loads the model, downloaded on the first run
    pub fn load() -> Result<Self> {
        let model = TextEmbedding::try_new(TextInitOptions::new(MODEL).with_show_download_progress(true))?;
        Ok(Embedder(Arc::new(Mutex::new(model))))
    }

    /// The vectors of texts built by `passage`
    pub async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let model = self.0.clone();
        actix_web::rt::task::spawn_blocking(move || {
            let mut model = model.lock().map_err(|_| Error::Other("the embedding model is poisoned".into()))?;
            model.embed(texts, Some(BATCH_SIZE))
        })
        .await
        .map_err(|e| Error::Other(e.to_string()))?
    }

    /// The vector of a search, nearest to the vectors of the texts answering it
    pub async fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        let mut vectors = self.embed(vec![format!("{}{}", QUERY_PREFIX, query)]).await?;
        vectors.pop().ok_or_else(|| Error::Other("the model returned no vector".into()))
    }

    /// The text of an entity, its parts joined, the missing and empty ones skipped
    pub fn passage(parts: &[Option<&str>]) -> String {
        let parts: Vec<&str> = parts.iter().flatten().map(|part| part.trim()).filter(|part| !part.is_empty()).collect();
        format!("{}{}", PASSAGE_PREFIX, parts.join(". "))
    }
}
"#;

// src/embedder.rs, shared by the searches
pub(crate) fn render_embedder(model: &EmbeddingModel) -> String {
    EMBEDDER_TPL
        .replace("{model}", model.name)
        .replace("{what}", model.what)
        .replace("{dimensions}", &model.dimensions.to_string())
        .replace("{query_prefix}", model.query_prefix)
        .replace("{passage_prefix}", model.passage_prefix)
}

const SEARCH_TPL: &str = r##"
// The search of the {plural} by the meaning of their text: `{field}` is computed from it by the embedding
// model of src/embedder.rs when a {entity_lower_case} is saved, and `GET /{path}/search?q=...` lists the {plural}
// whose `{field}` is nearest to the vector of `q`, with their similarity
use std::collections::HashMap;
use std::fmt;

use actix_web::http::StatusCode;
use actix_web::web::{Data, Json, Query};
use actix_web::{HttpResponse, ResponseError};
use octopux::pgvector::Vector;
use octopux::{anyhow, async_trait, BeforeSave, Model};
use serde::{Deserialize, Serialize};{openapi_imports}{graphql_imports}

// The application state, declared (or re-exported) at the root of the crate, with the embedder of the searches
use crate::AppState;
use crate::embedder::{Embedder, BATCH_SIZE};
use super::{module}::{{entity}, Id, ListQuery, New{entity}, Updatable{entity}};

/// The text `{field}` is computed from
pub fn {field}_text({text_params}) -> String {
    Embedder::passage(&[{text_parts}])
}

// Computes `{field}` before the insert, so that the searches find the {entity_lower_case} at once
#[async_trait]
impl BeforeSave<AppState> for New{entity} {
    async fn before_save(mut self: Self, state: &AppState) -> anyhow::Result<Self> {
        let text = {field}_text({self_args});
        self.{field_ident} = state.embedder.embed(vec![text]).await?.pop().map(Vector::from);
        Ok(self)
    }
}

// Computes `{field}` again from the new text before the update
#[async_trait]
impl BeforeSave<AppState> for Updatable{entity} {
    async fn before_save(mut self: Self, state: &AppState) -> anyhow::Result<Self> {
        let text = {field}_text({self_args});
        self.{field_ident} = state.embedder.embed(vec![text]).await?.pop().map(Vector::from);
        Ok(self)
    }
}

/// Computes `{field}` for the {plural} saved without it, before the search existed, by batches.
/// Returns the number of {plural} computed
pub async fn embed_missing(state: &AppState) -> anyhow::Result<usize> {
    let mut count = 0;
    loop {
        let rows: Vec<(Id, {row_types})> = sqlx::query_as("SELECT id, {columns} FROM {table} WHERE {column} IS NULL ORDER BY id LIMIT $1")
            .bind(BATCH_SIZE as i64)
            .fetch_all(&state.pool)
            .await?;
        if rows.is_empty() {
            return Ok(count);
        }
        let texts = rows.iter().map(|(_, {row_names})| {field}_text({row_args})).collect();
        let vectors: Vec<Vector> = state.embedder.embed(texts).await?.into_iter().map(Vector::from).collect();
        // one UPDATE for the batch, the ids and the vectors unnested side by side
        let ids: Vec<Id> = rows.iter().map(|row| row.0).collect();
        sqlx::query("UPDATE {table} AS t SET {column} = batch.vector FROM UNNEST($1::int8[], $2::vector[]) AS batch(id, vector) WHERE t.id = batch.id")
            .bind(&ids)
            .bind(&vectors)
            .execute(&state.pool)
            .await?;
        count += rows.len();
    }
}

#[derive(Deserialize{openapi_derives})]
pub struct SearchQuery {
    /// What to search, by meaning rather than by words
    pub q: String,
    /// Number of rows to skip
    pub offset: Option<usize>,
    /// Maximum number of rows to return (20 by default, 100 at most)
    pub limit: Option<usize>,
}

/// A {entity_lower_case} found by the search, and how similar it is to `q`: the cosine similarity of their
/// vectors, 1 for the same meaning, around {unrelated} for unrelated texts with {embedding_model}
#[derive(Serialize{openapi_derives}{graphql_derive})]
pub struct {entity}SearchResult {
    pub similarity: f64,
    pub {entity_field}: {entity},
}

/// An error of the search, answered as the errors of the generated routes: `{"code", "message"}`
#[derive(Debug{openapi_error_derive})]{openapi_error_attribute}
pub struct SearchError(anyhow::Error);

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#}", self.0)
    }
}

impl ResponseError for SearchError {
    fn status_code(&self) -> StatusCode {
        self.0.downcast_ref::<octopux::Error>().map_or(StatusCode::INTERNAL_SERVER_ERROR, |error| error.status_code())
    }

    fn error_response(&self) -> HttpResponse {
        match self.0.downcast_ref::<octopux::Error>() {
            Some(error) => error.error_response(),
            None => {
                eprintln!("search failed: {:#}", self.0);
                HttpResponse::InternalServerError()
                    .content_type("application/json")
                    .body(r#"{"code":"INTERNAL_ERROR","message":"an internal error occurred"}"#)
            }
        }
    }
}

impl From<anyhow::Error> for SearchError {
    fn from(error: anyhow::Error) -> Self {
        SearchError(error)
    }
}

/// Searches the {plural} by meaning
///
/// Computes the vector of `q` with the model of the {plural}, then lists the {plural} as `GET /{path}`,
/// the nearest to the search first, with their similarity
{api_operation}pub async fn search(query: Query<SearchQuery>, state: Data<AppState>) -> Result<Json<Vec<{entity}SearchResult>>, SearchError> {
    Ok(Json(search_{plural}(query.into_inner(), &state).await?))
}

/// The search, shared by `GET /{path}/search`{graphql_shared}
pub async fn search_{plural}(query: SearchQuery, state: &AppState) -> anyhow::Result<Vec<{entity}SearchResult>> {
    if query.q.trim().is_empty() {
        return Err(octopux::Error::BadRequest("`q` must tell what to search".into()).into());
    }
    let vector = Vector::from(state.embedder.embed_query(&query.q).await?);
    let list = ListQuery { offset: query.offset, limit: query.limit, near: Some(vector.clone()), ..Default::default() };
    let {plural} = {entity}::list(&list, state).await?;
    // the similarities of the page, 1 minus the cosine distance it is ordered by: the {plural} without
    // `{field}` have none and are left out (see `embed_missing`)
    let ids: Vec<Id> = {plural}.iter().map(|{entity_field}| {entity_field}.id).collect();
    let similarities: HashMap<Id, f64> = sqlx::query_as("SELECT id, 1 - ({column} <=> $1) FROM {table} WHERE id = ANY($2) AND {column} IS NOT NULL")
        .bind(&vector)
        .bind(&ids)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .collect();
    Ok({plural}
        .into_iter()
        .filter_map(|{entity_field}| Some({entity}SearchResult { similarity: *similarities.get(&{entity_field}.id)?, {entity_field} }))
        .collect())
}

// Registers `GET /{path}/search`, to mount before the routes of the {entity_lower_case} endpoint, so that `search`
// is not taken for the `{id}` of `GET /{path}/{id}`: `.configure({search_module}::configure).configure({module}::configure)`
pub fn configure(cfg: &mut {web}::ServiceConfig) {
    cfg.route("/{path}/search", {web}::get().to(search));
}{graphql_root}
"##;

const SEARCH_OPENAPI_IMPORTS: &str = "
use apistos::{api_operation, ApiComponent, ApiErrorComponent};
use schemars::JsonSchema;";

const SEARCH_OPENAPI_ERROR: &str = r#"
#[openapi_error(
    status(code = 400, description = "BAD_REQUEST: `q` is empty, or the query is invalid"),
    status(code = 500, description = "INTERNAL_ERROR: the embedding or the query failed")
)]"#;

const SEARCH_GRAPHQL_ROOT: &str = r#"

// The GraphQL search of the {plural} (async-graphql), to merge into the query root of the schema:
// `#[derive(MergedObject, Default)] struct Query({search_module}::{entity}SearchQuery, ...);`
#[derive(Default)]
pub struct {entity}SearchQuery;

#[Object]
impl {entity}SearchQuery {
    /// The {plural} nearest by meaning to `q`, with their similarity, as `GET /{path}/search`
    async fn search_{plural}(
        &self,
        ctx: &Context<'_>,
        q: String,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> async_graphql::Result<Vec<{entity}SearchResult>> {
        let state = ctx.data::<Data<AppState>>()?.get_ref();
        Ok(search_{plural}(SearchQuery { q, offset, limit }, state).await?)
    }
}"#;

// What the search file of a model is generated from
pub(crate) struct SearchFile<'a> {
    pub(crate) entity: &'a str,
    // the module of the model, `book_page`, a sibling of the module of the search
    pub(crate) module: &'a str,
    // the module of the search, `book_page_search`
    pub(crate) search_module: &'a str,
    // the table, qualified with its schema
    pub(crate) table: &'a str,
    pub(crate) openapi: bool,
    pub(crate) graphql: bool,
}

// `<table>_search.rs`, next to the model
pub(crate) fn render_search(file: &SearchFile, search: &Search) -> String {
    let text = &search.text;
    let join = |part: &dyn Fn(&str, bool) -> String| text.iter().map(|(name, optional)| part(&field_ident(name), *optional)).collect::<Vec<_>>().join(", ");
    let text_params = join(&|name, optional| format!("{}: {}", name, if optional { "Option<&str>" } else { "&str" }));
    let text_parts = join(&|name, optional| if optional { name.to_string() } else { format!("Some({})", name) });
    let self_args = join(&|name, optional| if optional { format!("self.{}.as_deref()", name) } else { format!("&self.{}", name) });
    let row_types = join(&|_, optional| if optional { "Option<String>".to_string() } else { "String".to_string() });
    let row_names = join(&|name, _| name.to_string());
    let row_args = join(&|name, optional| if optional { format!("{}.as_deref()", name) } else { name.to_string() });
    let columns = text.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>().join(", ");
    let plural = pluralize(&to_snake_case(file.entity));
    let (openapi_imports, openapi_derives, openapi_error_derive, openapi_error_attribute, api_operation, web) = if file.openapi {
        (
            SEARCH_OPENAPI_IMPORTS,
            ", JsonSchema, ApiComponent",
            ", ApiErrorComponent",
            SEARCH_OPENAPI_ERROR,
            format!("#[api_operation(tag = \"{}\", operation_id = \"search_{}\")]\n", file.entity.to_lowercase(), plural),
            "apistos::web",
        )
    } else {
        ("", "", "", "", String::new(), "actix_web::web")
    };
    let (graphql_imports, graphql_derive, graphql_shared, graphql_root) = if file.graphql {
        ("\nuse async_graphql::{Context, Object, SimpleObject};", ", SimpleObject", format!(" and the `search{}` GraphQL query", to_camel_case(&plural)), SEARCH_GRAPHQL_ROOT)
    } else {
        ("", "", String::new(), "")
    };
    SEARCH_TPL
        .replace("{openapi_imports}", openapi_imports)
        .replace("{openapi_derives}", openapi_derives)
        .replace("{openapi_error_derive}", openapi_error_derive)
        .replace("{openapi_error_attribute}", openapi_error_attribute)
        .replace("{api_operation}", &api_operation)
        .replace("{graphql_imports}", graphql_imports)
        .replace("{graphql_derive}", graphql_derive)
        .replace("{graphql_shared}", &graphql_shared)
        .replace("{graphql_root}", graphql_root)
        .replace("{web}", web)
        .replace("{text_params}", &text_params)
        .replace("{text_parts}", &text_parts)
        .replace("{self_args}", &self_args)
        .replace("{row_types}", &row_types)
        .replace("{row_names}", &row_names)
        .replace("{row_args}", &row_args)
        .replace("{columns}", &columns)
        .replace("{unrelated}", search.model.unrelated)
        .replace("{embedding_model}", search.model.name)
        .replace("{entity_field}", &field_ident(&to_snake_case(file.entity)))
        .replace("{column}", &search.field)
        .replace("{field_ident}", &field_ident(&search.field))
        .replace("{field}", &search.field)
        .replace("{table}", file.table)
        .replace("{search_module}", file.search_module)
        .replace("{module}", file.module)
        .replace("{plural}", &plural)
        .replace("{path}", &file.entity.to_lowercase())
        .replace("{entity_lower_case}", &file.entity.to_lowercase())
        .replace("{entity}", file.entity)
}

// The src/helpers.rs (or any file) `source` declaring `struct AppState` with the embedder of the searches,
// unchanged when it has one, None when it declares no such struct
pub(crate) fn with_embedder_state(source: &str) -> Option<String> {
    let file = syn::parse_file(source).ok()?;
    let state = file.items.iter().find_map(|item| match item {
        syn::Item::Struct(s) if s.ident == "AppState" => Some(s),
        _ => None,
    })?;
    let syn::Fields::Named(named) = &state.fields else { return None };
    if named.named.iter().any(|f| f.ident.as_ref().is_some_and(|i| i == "embedder")) {
        return Some(source.to_string());
    }
    let close = named.brace_token.span.close().byte_range().start;
    let mut edits = Vec::new();
    if let Some(last) = named.named.last().filter(|_| !named.named.trailing_punct()) {
        edits.push((last.span().byte_range().end, ",".to_string()));
    }
    let line = "pub embedder: crate::embedder::Embedder,";
    match line_indent(source, close) {
        (start, Some(indent)) => {
            let indent = match named.named.last().map(|f| line_indent(source, f.span().byte_range().start).1) {
                Some(Some(field_indent)) => field_indent.to_string(),
                _ => format!("{}    ", indent),
            };
            edits.push((start, format!("{}// the embedding model of the searches\n{}{}\n", indent, indent, line)));
        }
        (_, None) => edits.push((close, format!(" {} ", line))),
    }
    Some(apply_edits(source, edits))
}

// The src/main.rs of --bootstrap `source` loading the embedder into the state, unchanged when it does,
// None when it does not build the state with `AppState { pool }`
pub(crate) fn with_embedder_main(source: &str) -> Option<String> {
    if source.contains("mod embedder;") && source.contains("embedder::Embedder::load()") {
        return Some(source.to_string());
    }
    let state = source.find("AppState { pool }")?;
    // the embedder is loaded on the line before the one building the state, with its indentation
    let start = source[..state].rfind('\n').map_or(0, |i| i + 1);
    let line = &source[start..];
    let indent = &line[..line.len() - line.trim_start().len()];
    let load = format!(
        "{i}// the embedding model of the searches, downloaded on the first run (see src/embedder.rs)\n{i}let embedder = embedder::Embedder::load().expect(\"embedding model not loaded\");\n",
        i = indent
    );
    let mut patched = format!("{}{}{}", &source[..start], load, &source[start..]).replacen("AppState { pool }", "AppState { pool, embedder }", 1);
    if !patched.contains("mod embedder;") {
        patched = match patched.find("mod helpers;") {
            Some(at) => format!("{}mod embedder;\n{}", &patched[..at], &patched[at..]),
            None => format!("mod embedder;\n{}", patched),
        };
    }
    Some(patched)
}

// The source files the search of a model is written in, next to the model, and the files of the crate
// it loads the embedder in
pub(crate) struct SearchPaths {
    pub(crate) search: String,
    pub(crate) embedder: String,
    // the files declaring `AppState` (src/helpers.rs of --bootstrap) and building it (src/main.rs)
    pub(crate) state: String,
    pub(crate) main: String,
}

impl SearchPaths {
    // `model` is the path of the model file
    pub(crate) fn new(root: &Path, model: &str) -> SearchPaths {
        let model = Path::new(model);
        let stem = model.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        SearchPaths {
            search: model.with_file_name(format!("{}_search.rs", stem)).display().to_string(),
            embedder: source_path(root, None, "embedder.rs"),
            state: source_path(root, None, "helpers.rs"),
            main: source_path(root, None, "main.rs"),
        }
    }

    // The model of the existing src/embedder.rs, its dimensions being those of the vector
    pub(crate) fn embedder_model(&self, root: &Path) -> Option<&'static EmbeddingModel> {
        fs::read_to_string(root.join(&self.embedder)).ok().as_deref().and_then(embedder_model)
    }

    // The files written, and whether they exist, for the recap: the state and src/main.rs only when they change
    pub(crate) fn files(&self, root: &Path, file: &SearchFile) -> Vec<(String, bool)> {
        let exists = |path: &str| root.join(path).exists();
        let mut files = vec![(self.search.clone(), exists(&self.search))];
        if !exists(&self.embedder) {
            files.push((self.embedder.clone(), false));
        }
        if self.state_patch(root).is_some_and(|(source, patched)| source != patched) {
            files.push((self.state.clone(), true));
        }
        if !self.main_patch(root, file).1.is_empty() {
            files.push((self.main.clone(), true));
        }
        files
    }

    // The state file and the state file with the embedder, None when it declares no `AppState`
    fn state_patch(&self, root: &Path) -> Option<(String, String)> {
        let source = fs::read_to_string(root.join(&self.state)).ok()?;
        let patched = with_embedder_state(&source)?;
        Some((source, patched))
    }

    // The query root of the GraphQL search, as merged in src/main.rs
    fn graphql_root(&self, root: &Path, file: &SearchFile) -> String {
        let module = crate_module(root, Path::new(&self.search))
            .and_then(|m| m.strip_prefix("crate::").map(String::from))
            .unwrap_or_else(|| file.search_module.to_string());
        format!("{}::{}SearchQuery", module, file.entity)
    }

    // src/main.rs loading the embedder into the state and merging the GraphQL search, what it changes,
    // and what is left to do by hand
    fn main_patch(&self, root: &Path, file: &SearchFile) -> (Option<String>, Vec<&'static str>, Vec<String>) {
        let original = fs::read_to_string(root.join(&self.main)).ok();
        let mut main = original.clone();
        let (mut changes, mut hints) = (Vec::new(), Vec::new());
        match main.as_deref().and_then(with_embedder_main) {
            Some(patched) => {
                if Some(&patched) != main.as_ref() {
                    changes.push("loads the embedder into the state");
                }
                main = Some(patched);
            }
            None => hints.push(
                "  Declare `mod embedder;` at the root of the crate, and load the embedder into the AppState: `embedder: embedder::Embedder::load().expect(\"embedding model not loaded\")`".to_string(),
            ),
        }
        if file.graphql {
            let query = self.graphql_root(root, file);
            match main.as_deref().and_then(|source| with_merged_root(source, "Query", &query)) {
                Some(patched) => {
                    if Some(&patched) != main.as_ref() {
                        changes.push("merges the GraphQL search into the query root");
                    }
                    main = Some(patched);
                }
                None => hints.push(format!("  Merge `{}` into the query root of the async-graphql schema", query)),
            }
        }
        (main.filter(|_| !changes.is_empty()), changes, hints)
    }
}

// Writes the search file, src/embedder.rs when missing, loads the embedder in the state of src/helpers.rs and
// src/main.rs of --bootstrap and merges the GraphQL root of the search, then prints what is left to do by hand.
// `existing` is set when the table may already have rows, whose vectors `embed_missing` computes
pub(crate) fn write_search(root: &Path, paths: &SearchPaths, file: &SearchFile, search: &Search, existing: bool) -> Result<(), Error> {
    write_source(&paths.search, &render_search(file, search))?;
    println!("{}", success(&format!("Generated the search of {} in {}, {}", file.entity, paths.search, declaration_hint(root, &paths.search, file.search_module))));
    println!(
        "{}",
        highlight(&format!(
            "  Mount it before the routes of the model, so that `/{p}/search` is not taken for a `/{p}/{{id}}`: `.configure({s}::configure).configure({m}::configure)`",
            p = file.entity.to_lowercase(),
            s = file.search_module,
            m = file.module
        ))
    );
    if !root.join(&paths.embedder).exists() {
        write_source(&paths.embedder, &render_embedder(search.model))?;
        println!("{}", success(&format!("Generated {} computing the vectors with {}", paths.embedder, search.model.name)));
    }
    match paths.state_patch(root) {
        Some((source, patched)) if source == patched => {}
        Some((_, patched)) => {
            fs::write(root.join(&paths.state), patched)?;
            println!("{}", success(&format!("Added the embedder to the AppState of {}", paths.state)));
        }
        None => println!("{}", highlight("  Add the embedder to the AppState: `pub embedder: crate::embedder::Embedder,`")),
    }
    let (main, changes, hints) = paths.main_patch(root, file);
    if let Some(main) = main {
        fs::write(root.join(&paths.main), main)?;
        println!("{}", success(&format!("{} {}", paths.main, changes.join(" and "))));
    }
    for hint in hints {
        println!("{}", highlight(&hint));
    }
    if existing {
        println!(
            "{}",
            highlight(&format!("  Compute the vectors of the existing {} once the state is built: `{}::embed_missing(&state).await`", pluralize(&to_snake_case(file.entity)), file.search_module))
        );
    }
    if !fs::read_to_string(root.join("Cargo.toml")).is_ok_and(|cargo| cargo.contains("fastembed")) {
        println!("{}", highlight(&format!("  src/embedder.rs needs fastembed: `{}`", FASTEMBED_ADD)));
    }
    println!("{}", highlight(&format!("  {} is downloaded from Hugging Face on the first run into `.fastembed_cache`", search.model.name)));
    Ok(())
}

const FASTEMBED_ADD: &str = "cargo add fastembed@7 --no-default-features --features ort-download-binaries-rustls-tls,hf-hub-rustls-tls";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{render_model, with_header, Dialect};

    fn field(name: &str, ty: &str) -> Field {
        Field { name: name.into(), ty: ty.into(), references: None, unique: false, length: None }
    }

    fn search() -> Search {
        Search { field: "embedding".into(), text: vec![("title".into(), false), ("summary".into(), true)], model: &EMBEDDING_MODELS[0] }
    }

    fn model(openapi: bool, graphql: bool) -> String {
        let fields = [field("title", "String"), field("summary", "Option<String>"), field("pages", "i32")];
        with_header("//", &render_model("BookPage", openapi, graphql, true, true, &fields, Dialect::Postgres))
    }

    #[test]
    fn the_dense_vectors_and_the_texts_are_recognized() {
        assert!(is_dense_vector("pgvector::Vector"));
        assert!(is_dense_vector("Option< octopux::pgvector::Vector >"));
        assert!(!is_dense_vector("pgvector::HalfVector"));
        assert!(!is_dense_vector("Option<pgvector::SparseVector>"));
        assert_eq!(text_type("String"), Some(false));
        assert_eq!(text_type("Option<String>"), Some(true));
        assert_eq!(text_type("i32"), None);
    }

    #[test]
    fn read_search_asks_the_vector_the_model_and_the_text() {
        let fields = [field("title", "String"), field("embedding", "pgvector::Vector")];
        let texts = [("title".to_string(), false), ("summary".to_string(), true)];
        let mut output = Vec::new();
        let search = read_search(&mut "y\n3\nsummary, nope\nsummary\n".as_bytes(), &mut output, &fields, &texts, None).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert_eq!(search, Some(Search { field: "embedding".into(), text: vec![("summary".into(), true)], model: &EMBEDDING_MODELS[2] }));
        assert!(output.contains("`nope` is not a text field"), "{}", output);
        // the model of the existing embedder, all the text fields by default
        let search = read_search(&mut "y\n\n".as_bytes(), &mut Vec::new(), &fields, &texts, Some(&EMBEDDING_MODELS[1])).unwrap().unwrap();
        assert_eq!((search.model.name, search.text.len()), ("MultilingualE5Base", 2));
        assert_eq!(read_search(&mut "\n".as_bytes(), &mut Vec::new(), &fields, &texts, None).unwrap(), None);
        assert_eq!(read_search(&mut "y\n".as_bytes(), &mut Vec::new(), &fields, &[], None).unwrap(), None);
    }

    #[test]
    fn the_vector_of_the_search_is_optional_and_of_the_dimensions_of_the_model() {
        let vector = search_field(&field("embedding", "pgvector::Vector"), &EMBEDDING_MODELS[1]);
        assert_eq!((vector.ty.as_str(), vector.length), ("Option<pgvector::Vector>", Some(768)));
        assert_eq!(Dialect::Postgres.column_type(&vector.ty, vector.length).as_deref(), Some("vector(768)"));
    }

    #[test]
    fn the_model_is_patched_for_the_search() {
        let source = model(true, true);
        assert_eq!(model_text_fields(&source, "BookPage").unwrap(), [("title".to_string(), false), ("summary".to_string(), true)]);
        assert!(!has_before_save(&source, "BookPage"));
        let vector = search_field(&field("embedding", "pgvector::Vector"), &EMBEDDING_MODELS[0]);
        let patched = with_search_field(&source, "BookPage", &vector).unwrap();
        let patched = with_search(&patched, "BookPage", &search(), "pgvector::Vector").unwrap();
        syn::parse_file(&patched).unwrap();
        assert!(patched.contains("use octopux::pgvector;"), "{}", patched);
        // the model struct is sent without the vector, the payloads never carry it
        assert_eq!(patched.matches("pub embedding: Option<pgvector::Vector>,").count(), 2, "{}", patched);
        assert_eq!(patched.matches("#[graphql(skip)]").count(), 2, "{}", patched);
        assert!(patched.contains("#[sqlx_model(database = \"postgres\", timestamps, soft_delete, filter)]"), "{}", patched);
        assert_eq!(patched.matches("before_save)]").count(), 2, "{}", patched);
        assert!(patched.contains("#[derive(Default, Deserialize, JsonSchema, ApiComponent, SqlxFilter)]\n    #[sqlx_filter(database = \"postgres\")]\n    pub struct ListQuery"), "{}", patched);
        assert!(patched.contains("#[sqlx_filter(column = \"embedding\", op = \"nearest\")]\n        #[serde(skip)]\n        #[schemars(skip)]\n        pub near: Option<pgvector::Vector>,\n    }"), "{}", patched);
        // imported once, by the model or by the search
        assert_eq!(patched.matches("        SqlxFilter,\n").count(), 1, "{}", patched);
        assert!(patched.contains("ListQuery { offset, limit, sort, order, ..Default::default() }"), "{}", patched);
        assert!(has_before_save(&patched, "BookPage"));
        assert!(with_search(&patched, "BookPage", &search(), "pgvector::Vector").unwrap_err().contains("already declares a `near`"));
    }

    #[test]
    fn the_search_file_embeds_the_text_fields() {
        let file = SearchFile { entity: "BookPage", module: "book_page", search_module: "book_page_search", table: "app.book_page", openapi: true, graphql: true };
        let rendered = render_search(&file, &search());
        syn::parse_file(&rendered).unwrap();
        for expected in [
            "pub fn embedding_text(title: &str, summary: Option<&str>) -> String {\n    Embedder::passage(&[Some(title), summary])",
            "let text = embedding_text(&self.title, self.summary.as_deref());",
            "use super::book_page::{BookPage, Id, ListQuery, NewBookPage, UpdatableBookPage};",
            "let rows: Vec<(Id, String, Option<String>)> = sqlx::query_as(\"SELECT id, title, summary FROM app.book_page WHERE embedding IS NULL",
            "embedding_text(title, summary.as_deref())",
            "UPDATE app.book_page AS t SET embedding = batch.vector",
            "#[api_operation(tag = \"bookpage\", operation_id = \"search_book_pages\")]\npub async fn search(",
            "cfg.route(\"/bookpage/search\", apistos::web::get().to(search));",
            "pub struct BookPageSearchQuery;",
            "async fn search_book_pages(\n        &self,\n        ctx: &Context<'_>,\n        q: String,\n        offset: Option<usize>,",
            ".configure(book_page_search::configure).configure(book_page::configure)",
            "#[derive(Serialize, JsonSchema, ApiComponent, SimpleObject)]\npub struct BookPageSearchResult {\n    pub similarity: f64,\n    pub book_page: BookPage,\n}",
            "around 0.8 for unrelated texts with MultilingualE5Small",
            "-> Result<Json<Vec<BookPageSearchResult>>, SearchError>",
            "sqlx::query_as(\"SELECT id, 1 - (embedding <=> $1) FROM app.book_page WHERE id = ANY($2) AND embedding IS NOT NULL\")",
            "Some(BookPageSearchResult { similarity: *similarities.get(&book_page.id)?, book_page })",
            "Ok(search_book_pages(SearchQuery { q, offset, limit }, state).await?)",
        ] {
            assert!(rendered.contains(expected), "`{}` missing from\n{}", expected, rendered);
        }
        let plain = render_search(&SearchFile { openapi: false, graphql: false, ..file }, &search());
        syn::parse_file(&plain).unwrap();
        assert!(plain.contains("cfg.route(\"/bookpage/search\", actix_web::web::get().to(search));"), "{}", plain);
        assert!(!plain.contains("apistos") && !plain.contains("async_graphql"), "{}", plain);
    assert!(plain.contains("#[derive(Serialize)]\npub struct BookPageSearchResult {"), "{}", plain);
    }

    #[test]
    fn the_embedder_only_depends_on_fastembed() {
        let rendered = render_embedder(&EMBEDDING_MODELS[0]);
        syn::parse_file(&rendered).unwrap();
        assert!(!rendered.contains("use octopux"), "{}", rendered);
        assert!(rendered.contains("pub const MODEL: EmbeddingModel = EmbeddingModel::MultilingualE5Small;"), "{}", rendered);
        assert!(rendered.contains("const QUERY_PREFIX: &str = \"query: \";"), "{}", rendered);
        assert_eq!(embedder_model(&rendered), Some(&EMBEDDING_MODELS[0]));
        assert_eq!(embedder_model("use fastembed::EmbeddingModel::JinaEmbeddingsV2BaseCode;"), None);
    }

    #[test]
    fn the_bootstrapped_state_and_main_load_the_embedder() {
        let helpers = "use sqlx::PgPool;\n\npub struct AppState {\n    pub pool: PgPool,\n}\n";
        let patched = with_embedder_state(helpers).unwrap();
        assert_eq!(patched, "use sqlx::PgPool;\n\npub struct AppState {\n    pub pool: PgPool,\n    // the embedding model of the searches\n    pub embedder: crate::embedder::Embedder,\n}\n");
        assert_eq!(with_embedder_state(&patched).unwrap(), patched);
        assert_eq!(with_embedder_state("pub struct Other {}"), None);
        let main = crate::render_bootstrap_main(true, false, Dialect::Postgres);
        let patched = with_embedder_main(&main).unwrap();
        assert!(patched.starts_with("mod embedder;\nmod helpers;"), "{}", patched);
        assert!(patched.contains("    let embedder = embedder::Embedder::load().expect(\"embedding model not loaded\");\n    let state = web::Data::new(AppState { pool, embedder });"), "{}", patched);
        assert_eq!(with_embedder_main(&patched).unwrap(), patched);
        assert_eq!(with_embedder_main("fn main() {}"), None);
    }
}
