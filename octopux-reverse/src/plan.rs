// The octopux models and relations of a schema, and the `octopux` commands generating them
use crate::schema::{Dialect, Table};
use std::collections::BTreeSet;

const TIMESTAMP_COLUMNS: [&str; 3] = ["created_at", "updated_at", "deleted_at"];
// Type of the timestamps declared by `octopux generate-model --timestamps`
const TIMESTAMP_TYPE: &str = "DateTime<Utc>";

// The Rust type a column decodes into with sqlx, None when octopux has no field type for it
pub fn rust_type(dialect: Dialect, sql_type: &str) -> Option<String> {
    let ty = match dialect {
        Dialect::Sqlite => sqlite_type(sql_type),
        Dialect::Postgres => return postgres_type(sql_type),
        Dialect::Mysql => mysql_type(sql_type),
    };
    ty.map(str::to_string)
}

// From the declared type, after the affinity rules of SQLite (https://sqlite.org/datatype3.html),
// the types sqlx decodes as booleans and dates first
fn sqlite_type(declared: &str) -> Option<&'static str> {
    let ty = declared.to_uppercase();
    let ty = ty.as_str();
    Some(match ty {
        _ if ty.contains("BOOL") => "bool",
        _ if ty.contains("INT") => "i64",
        _ if ty.starts_with("DATETIME") || ty.starts_with("TIMESTAMP") => TIMESTAMP_TYPE,
        "DATE" => "NaiveDate",
        "TIME" => "NaiveTime",
        _ if ty.contains("CHAR") || ty.contains("CLOB") || ty.contains("TEXT") => "String",
        _ if ty.contains("BLOB") => "Vec<u8>",
        _ if ty.contains("REAL") || ty.contains("FLOA") || ty.contains("DOUB") || ty.contains("NUMERIC") || ty.contains("DECIMAL") => "f64",
        // a column without type holds any value
        _ => return None,
    })
}

// From the udt name, `_int4` being an array of `int4`
fn postgres_type(udt: &str) -> Option<String> {
    if let Some(element) = udt.strip_prefix('_') {
        return postgres_type(element).filter(|ty| !ty.starts_with("Vec<") || ty == "Vec<u8>").map(|ty| format!("Vec<{}>", ty));
    }
    Some(
        match udt {
            "int2" => "i16",
            "int4" => "i32",
            "int8" => "i64",
            "float4" => "f32",
            "float8" => "f64",
            "bool" => "bool",
            "text" | "varchar" | "bpchar" | "name" | "citext" => "String",
            "timestamptz" => TIMESTAMP_TYPE,
            "timestamp" => "NaiveDateTime",
            "date" => "NaiveDate",
            "time" => "NaiveTime",
            "bytea" => "Vec<u8>",
            // need the matching sqlx features and crates
            "uuid" => "uuid::Uuid",
            "numeric" => "rust_decimal::Decimal",
            "json" | "jsonb" => "serde_json::Value",
            _ => return None,
        }
        .to_string(),
    )
}

// From the column type: `int unsigned`, `tinyint(1)`, `varchar(255)`
fn mysql_type(column_type: &str) -> Option<&'static str> {
    let ty = column_type.to_lowercase();
    let unsigned = ty.contains("unsigned");
    let base = ty.split(|c: char| c == '(' || c == ' ').next().unwrap_or_default();
    Some(match (base, unsigned) {
        _ if ty.starts_with("tinyint(1)") && !unsigned => "bool",
        ("bool" | "boolean", _) => "bool",
        ("tinyint", false) => "i8",
        ("tinyint", true) => "u8",
        ("smallint", false) => "i16",
        ("smallint", true) => "u16",
        ("mediumint" | "int" | "integer", false) => "i32",
        ("mediumint" | "int" | "integer", true) => "u32",
        ("bigint", false) => "i64",
        ("bigint", true) => "u64",
        ("float", _) => "f32",
        ("double" | "real", _) => "f64",
        ("char" | "varchar" | "tinytext" | "text" | "mediumtext" | "longtext" | "enum" | "set", _) => "String",
        ("datetime" | "timestamp", _) => TIMESTAMP_TYPE,
        ("date", _) => "NaiveDate",
        ("time", _) => "NaiveTime",
        ("binary" | "varbinary" | "tinyblob" | "blob" | "mediumblob" | "longblob", _) => "Vec<u8>",
        ("decimal" | "numeric", _) => "rust_decimal::Decimal",
        ("json", _) => "serde_json::Value",
        _ => return None,
    })
}

// Whether the `id` column decodes into the `Id` (i64) of the octopux models
fn is_id_type(dialect: Dialect, sql_type: &str) -> bool {
    rust_type(dialect, sql_type).as_deref() == Some("i64")
}

// snake_case names, as the octopux CLI accepts them for the fields, the tables and the relations
pub fn is_snake_case(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() || c == '_' => {
            chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') && !["_", "self", "super", "crate"].contains(&name)
        }
        _ => false,
    }
}

// `book_page` gives `BookPage`
pub fn to_camel_case(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or(String::new(), |c| c.to_uppercase().chain(chars).collect())
        })
        .collect()
}

// `BookPage` gives `book_page`, as the octopux CLI names the modules (the models have no acronyms here)
pub fn to_snake_case(name: &str) -> String {
    let mut snake = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            snake.push('_');
        }
        snake.extend(c.to_lowercase());
    }
    snake
}

// Singular of the last word of a snake_case table name: `categories` gives `category`, `books` gives `book`
pub fn singularize(name: &str) -> String {
    if let Some(stem) = name.strip_suffix("ies").filter(|s| !s.is_empty() && !s.ends_with('_')) {
        return format!("{}y", stem);
    }
    for end in ["sses", "xes", "zes", "ches", "shes"] {
        if name.ends_with(end) {
            return name[..name.len() - 2].to_string();
        }
    }
    if name.ends_with('s') && !["ss", "us", "is", "_s"].iter().any(|end| name.ends_with(end)) && name.len() > 1 {
        return name[..name.len() - 1].to_string();
    }
    name.to_string()
}

// Plural of the last word of a snake_case name, as the octopux CLI names the relations
pub fn pluralize(name: &str) -> String {
    let consonant_y = name.ends_with('y') && !name[..name.len() - 1].ends_with(['a', 'e', 'i', 'o', 'u']);
    if consonant_y {
        format!("{}ies", &name[..name.len() - 1])
    } else if ["s", "x", "z", "ch", "sh"].iter().any(|end| name.ends_with(end)) {
        format!("{}es", name)
    } else {
        format!("{}s", name)
    }
}

#[derive(Debug, PartialEq)]
pub struct Model {
    pub name: String,
    pub table: String,
    // name and type of the columns besides `id` and the timestamps
    pub fields: Vec<(String, String)>,
    // the table has the `created_at`, `updated_at` and `deleted_at` columns of `--timestamps`
    pub timestamps: bool,
}

impl Model {
    // the octopux CLI names the file and the module after the table
    pub fn module(&self) -> String {
        self.table.clone()
    }
}

#[derive(Debug, PartialEq)]
pub struct Through {
    pub model: String,
    pub table: String,
    pub child_key: String,
}

// A has-many relation of `parent`: the rows of `child` whose `foreign_key` references it, directly or through a join table
#[derive(Debug, PartialEq)]
pub struct Relation {
    pub parent: String,
    pub parent_table: String,
    pub child: String,
    pub child_table: String,
    pub name: String,
    pub foreign_key: String,
    pub through: Option<Through>,
    // every table of the relation has a `deleted_at` column, the soft deleted rows are skipped
    pub timestamps: bool,
}

impl Relation {
    pub fn module(&self) -> String {
        format!("{}_{}", self.parent_table, self.name)
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Plan {
    pub models: Vec<Model>,
    pub relations: Vec<Relation>,
    // what is left out, and why
    pub warnings: Vec<String>,
}

// The model of a table, or why it is not one
fn model(table: &Table, dialect: Dialect, warnings: &mut Vec<String>) -> Result<Model, String> {
    if !is_snake_case(&table.name) {
        return Err("its name is not snake_case".to_string());
    }
    let primary_key: Vec<&str> = table.columns.iter().filter(|c| c.primary_key).map(|c| c.name.as_str()).collect();
    match (primary_key.as_slice(), table.column("id")) {
        (["id"], Some(id)) if is_id_type(dialect, &id.sql_type) => {}
        (["id"], Some(id)) => return Err(format!("its `id` is a {}, the octopux models need a 64 bit integer", id.sql_type)),
        _ => return Err("its primary key is not an `id` column".to_string()),
    }
    let timestamps = TIMESTAMP_COLUMNS
        .iter()
        .all(|name| table.column(name).map_or(false, |c| rust_type(dialect, &c.sql_type).as_deref() == Some(TIMESTAMP_TYPE)));
    let mut fields = Vec::new();
    for column in &table.columns {
        if column.name == "id" || (timestamps && TIMESTAMP_COLUMNS.contains(&column.name.as_str())) {
            continue;
        }
        if !is_snake_case(&column.name) {
            warnings.push(format!("{}.{}: left out, its name is not snake_case", table.name, column.name));
            continue;
        }
        match rust_type(dialect, &column.sql_type) {
            Some(ty) => fields.push((column.name.clone(), if column.nullable { format!("Option<{}>", ty) } else { ty })),
            None => warnings.push(format!("{}.{}: left out, no field type for {}", table.name, column.name, column.sql_type)),
        }
    }
    if fields.is_empty() {
        return Err("it has no column besides `id` and the timestamps".to_string());
    }
    Ok(Model { name: String::new(), table: table.name.clone(), fields, timestamps })
}

// The models of the tables, then the has-many relations of their foreign keys referencing an `id`,
// and the many-to-many relations of the join tables (only two foreign keys to the `id` of models)
pub fn plan(tables: &[Table], dialect: Dialect) -> Plan {
    let mut plan = Plan::default();
    for table in tables {
        match model(table, dialect, &mut plan.warnings) {
            Ok(model) => plan.models.push(model),
            Err(reason) => plan.warnings.push(format!("{}: no model, {}", table.name, reason)),
        }
    }
    name_models(&mut plan.models);

    let model_of = |table: &str| plan.models.iter().find(|m| m.table == table);
    let soft_deletes = |table: &str| tables.iter().find(|t| t.name == table).map_or(false, |t| t.column("deleted_at").is_some());
    let mut relations = Vec::new();
    for table in tables {
        // the keys referencing the id of a model
        let keys: Vec<_> = table
            .foreign_keys
            .iter()
            .filter(|k| k.references == "id" && model_of(&k.table).is_some() && is_snake_case(&k.column))
            .collect();
        if let Some(child) = model_of(&table.name) {
            for key in &keys {
                let parent = model_of(&key.table).unwrap();
                relations.push(Relation {
                    parent: parent.name.clone(),
                    parent_table: parent.table.clone(),
                    child: child.name.clone(),
                    child_table: child.table.clone(),
                    name: pluralize(&to_snake_case(&child.name)),
                    foreign_key: key.column.clone(),
                    through: None,
                    timestamps: soft_deletes(&parent.table) && soft_deletes(&child.table),
                });
            }
        }
        // a join table only holds its two keys, besides an `id` and the timestamps
        let joins = keys.len() == 2
            && keys[0].column != keys[1].column
            && table.columns.iter().all(|c| {
                c.name == "id" || TIMESTAMP_COLUMNS.contains(&c.name.as_str()) || keys.iter().any(|k| k.column == c.name)
            });
        if joins && is_snake_case(&table.name) {
            for (from, to) in [(keys[0], keys[1]), (keys[1], keys[0])] {
                let (parent, child) = (model_of(&from.table).unwrap(), model_of(&to.table).unwrap());
                relations.push(Relation {
                    parent: parent.name.clone(),
                    parent_table: parent.table.clone(),
                    child: child.name.clone(),
                    child_table: child.table.clone(),
                    name: pluralize(&to_snake_case(&child.name)),
                    foreign_key: from.column.clone(),
                    through: Some(Through {
                        model: model_of(&table.name).map_or_else(|| to_camel_case(&table.name), |m| m.name.clone()),
                        table: table.name.clone(),
                        child_key: to.column.clone(),
                    }),
                    timestamps: soft_deletes(&parent.table) && soft_deletes(&child.table) && soft_deletes(&table.name),
                });
            }
        }
    }
    name_relations(&mut relations, &plan.models, &mut plan.warnings);
    plan.relations = relations;
    plan
}

// Names the models after their singular table, `books` gives `Book`,
// the plain table name is kept when two tables would give the same model
fn name_models(models: &mut [Model]) {
    let singular: Vec<String> = models.iter().map(|m| to_camel_case(&singularize(&m.table))).collect();
    for (i, model) in models.iter_mut().enumerate() {
        let taken = singular.iter().enumerate().any(|(j, name)| j != i && *name == singular[i]);
        model.name = if taken { to_camel_case(&model.table) } else { singular[i].clone() };
    }
}

// Two relations of a parent cannot share a route, nor a relation share the module of a model:
// the name of the next ones gets the foreign key (`books_by_editor`), the relation is left out when it is still taken
fn name_relations(relations: &mut Vec<Relation>, models: &[Model], warnings: &mut Vec<String>) {
    let mut modules: BTreeSet<String> = models.iter().map(Model::module).collect();
    relations.retain_mut(|relation| {
        if !modules.contains(&relation.module()) {
            modules.insert(relation.module());
            return true;
        }
        let key = relation.foreign_key.strip_suffix("_id").unwrap_or(&relation.foreign_key);
        relation.name = format!("{}_by_{}", relation.name, key);
        if modules.insert(relation.module()) {
            return true;
        }
        warnings.push(format!("{}: relation {} of {} left out, its module is taken", relation.module(), relation.name, relation.parent));
        false
    });
}

// An `octopux` command, with the answers piped to its prompts
#[derive(Debug, PartialEq)]
pub struct Command {
    pub args: Vec<String>,
    pub stdin: Option<String>,
}

// Options passed on to every `octopux` command
#[derive(Debug, Default, Clone, Copy)]
pub struct Options {
    pub openapi: bool,
    pub graphql: bool,
    pub force: bool,
}

fn common_flags(args: &mut Vec<String>, dialect: Dialect, timestamps: bool, options: Options) {
    args.push("--sqlx".to_string());
    args.push(dialect.flag().to_string());
    for (flag, on) in [("--timestamps", timestamps), ("--openapi", options.openapi), ("--graphql", options.graphql), ("--force", options.force)] {
        if on {
            args.push(flag.to_string());
        }
    }
}

// The `generate-model` command of each model, then the `generate-relation` command of each relation,
// the fields are piped to `--fields` as `name:Type` lines, followed by the empty name ending them and the confirmation
pub fn commands(plan: &Plan, dialect: Dialect, options: Options) -> Vec<Command> {
    let mut commands = Vec::new();
    for model in &plan.models {
        let mut args: Vec<String> = ["generate-model", "--name", &model.name, "--table", &model.table, "--fields"].map(String::from).to_vec();
        common_flags(&mut args, dialect, model.timestamps, options);
        let mut stdin: String = model.fields.iter().map(|(name, ty)| format!("{}:{}\n", name, ty)).collect();
        stdin.push_str("\ny\n");
        commands.push(Command { args, stdin: Some(stdin) });
    }
    for relation in &plan.relations {
        let mut args: Vec<String> = [
            "generate-relation",
            "--parent",
            &relation.parent,
            "--child",
            &relation.child,
            "--name",
            &relation.name,
            "--foreign-key",
            &relation.foreign_key,
            "--parent-table",
            &relation.parent_table,
            "--child-table",
            &relation.child_table,
        ]
        .map(String::from)
        .to_vec();
        if let Some(through) = &relation.through {
            args.extend(["--through", &through.model, "--through-table", &through.table, "--child-key", &through.child_key].map(String::from));
        }
        common_flags(&mut args, dialect, relation.timestamps, options);
        commands.push(Command { args, stdin: None });
    }
    commands
}

// How to mount the generated models and relations in `src/main.rs`
pub fn mounting(plan: &Plan) -> String {
    let modules: Vec<String> = plan.models.iter().map(Model::module).chain(plan.relations.iter().map(Relation::module)).collect();
    let mut lines = vec!["Declare the modules in src/main.rs:".to_string()];
    lines.extend(modules.iter().map(|m| format!("    mod {};", m)));
    lines.push("and mount them in the scope of the routes:".to_string());
    lines.extend(modules.iter().map(|m| format!("    .configure({}::configure)", m)));
    lines.join("\n")
}

// The commands as a shell script, the piped answers in quoted here-documents
pub fn script(commands: &[Command], octopux: &str) -> String {
    let mut script = String::from("#!/bin/sh\n# Generated by octopux-reverse\nset -e\n");
    for command in commands {
        script.push('\n');
        script.push_str(octopux);
        for arg in &command.args {
            script.push(' ');
            script.push_str(arg);
        }
        match &command.stdin {
            Some(stdin) => {
                script.push_str(" <<'EOF'\n");
                script.push_str(stdin);
                script.push_str("EOF\n");
            }
            None => script.push('\n'),
        }
    }
    script
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Column, ForeignKey};

    fn column(name: &str, sql_type: &str, nullable: bool) -> Column {
        Column { name: name.into(), sql_type: sql_type.into(), nullable, primary_key: name == "id" }
    }

    fn table(name: &str, columns: Vec<Column>, keys: &[(&str, &str)]) -> Table {
        Table {
            name: name.into(),
            columns,
            foreign_keys: keys.iter().map(|(c, t)| ForeignKey { column: c.to_string(), table: t.to_string(), references: "id".into() }).collect(),
        }
    }

    fn id() -> Column {
        column("id", "int8", false)
    }

    #[test]
    fn column_types_map_to_field_types() {
        assert_eq!(rust_type(Dialect::Sqlite, "INTEGER").as_deref(), Some("i64"));
        assert_eq!(rust_type(Dialect::Sqlite, "BOOLEAN").as_deref(), Some("bool"));
        assert_eq!(rust_type(Dialect::Sqlite, "DATETIME").as_deref(), Some("DateTime<Utc>"));
        assert_eq!(rust_type(Dialect::Sqlite, "varchar(80)").as_deref(), Some("String"));
        assert_eq!(rust_type(Dialect::Sqlite, ""), None);
        assert_eq!(rust_type(Dialect::Postgres, "_int4").as_deref(), Some("Vec<i32>"));
        assert_eq!(rust_type(Dialect::Postgres, "_bytea").as_deref(), Some("Vec<Vec<u8>>"));
        assert_eq!(rust_type(Dialect::Postgres, "uuid").as_deref(), Some("uuid::Uuid"));
        assert_eq!(rust_type(Dialect::Postgres, "my_enum"), None);
        assert_eq!(rust_type(Dialect::Mysql, "tinyint(1)").as_deref(), Some("bool"));
        assert_eq!(rust_type(Dialect::Mysql, "int unsigned").as_deref(), Some("u32"));
        assert_eq!(rust_type(Dialect::Mysql, "varchar(255)").as_deref(), Some("String"));
        assert_eq!(rust_type(Dialect::Mysql, "datetime(6)").as_deref(), Some("DateTime<Utc>"));
    }

    #[test]
    fn names_are_singularized_and_camel_cased() {
        assert_eq!(singularize("books"), "book");
        assert_eq!(singularize("categories"), "category");
        assert_eq!(singularize("boxes"), "box");
        assert_eq!(singularize("status"), "status");
        assert_eq!(singularize("address"), "address");
        assert_eq!(singularize("book_page"), "book_page");
        assert_eq!(to_camel_case("book_page"), "BookPage");
        assert_eq!(to_snake_case("BookPage"), "book_page");
        assert_eq!(pluralize("category"), "categories");
    }

    #[test]
    fn tables_with_an_id_become_models() {
        let tables = [
            table("authors", vec![id(), column("name", "varchar", false), column("bio", "text", true)], &[]),
            table("tag", vec![column("id", "int4", false), column("label", "text", false)], &[]),
            table("settings", vec![column("key", "text", false), column("value", "text", true)], &[]),
            table("empty", vec![id()], &[]),
        ];
        let plan = plan(&tables, Dialect::Postgres);
        assert_eq!(
            plan.models,
            [Model {
                name: "Author".into(),
                table: "authors".into(),
                fields: vec![("name".into(), "String".into()), ("bio".into(), "Option<String>".into())],
                timestamps: false,
            }]
        );
        assert_eq!(
            plan.warnings,
            [
                "tag: no model, its `id` is a int4, the octopux models need a 64 bit integer",
                "settings: no model, its primary key is not an `id` column",
                "empty: no model, it has no column besides `id` and the timestamps",
            ]
        );
    }

    #[test]
    fn timestamps_columns_give_the_timestamps_option() {
        let columns = vec![
            id(),
            column("title", "text", false),
            column("created_at", "timestamptz", true),
            column("updated_at", "timestamptz", true),
            column("deleted_at", "timestamptz", true),
            column("Weird", "text", true),
            column("location", "geometry", true),
        ];
        let plan = plan(&[table("post", columns, &[])], Dialect::Postgres);
        assert!(plan.models[0].timestamps);
        assert_eq!(plan.models[0].fields, [("title".to_string(), "String".to_string())]);
        assert_eq!(plan.warnings, ["post.Weird: left out, its name is not snake_case", "post.location: left out, no field type for geometry"]);
        // a timestamp of another type stays a field
        let columns = vec![id(), column("created_at", "timestamp", true), column("updated_at", "timestamptz", true), column("deleted_at", "timestamptz", true)];
        assert!(!plan_models(columns)[0].timestamps);
    }

    fn plan_models(columns: Vec<Column>) -> Vec<Model> {
        plan(&[table("post", columns, &[])], Dialect::Postgres).models
    }

    #[test]
    fn foreign_keys_give_has_many_and_many_to_many_relations() {
        let tables = [
            table("project", vec![id(), column("name", "text", false), column("parent_id", "int8", true)], &[("parent_id", "project")]),
            table("books", vec![id(), column("title", "text", false), column("project_id", "int8", false), column("editor_id", "int8", false)], &[
                ("project_id", "project"),
                ("editor_id", "project"),
            ]),
            table("category", vec![id(), column("label", "text", false)], &[]),
            // a join table without id is not a model, but joins the relation
            table("project_category", vec![column("project_id", "int8", false), column("category_id", "int8", false)], &[
                ("project_id", "project"),
                ("category_id", "category"),
            ]),
        ];
        let plan = plan(&tables, Dialect::Postgres);
        let relations: Vec<(String, String, String, Option<&str>)> = plan
            .relations
            .iter()
            .map(|r| (r.module(), r.child.clone(), r.foreign_key.clone(), r.through.as_ref().map(|t| t.child_key.as_str())))
            .collect();
        assert_eq!(
            relations,
            [
                ("project_projects".into(), "Project".into(), "parent_id".into(), None),
                ("project_books".into(), "Book".into(), "project_id".into(), None),
                // the second relation of the same parent and child gets its foreign key
                ("project_books_by_editor".into(), "Book".into(), "editor_id".into(), None),
                ("project_categories".into(), "Category".into(), "project_id".into(), Some("category_id")),
                ("category_projects".into(), "Project".into(), "category_id".into(), Some("project_id")),
            ]
        );
        assert_eq!(plan.relations[1].child_table, "books");
        assert_eq!(plan.relations[3].through, Some(Through { model: "ProjectCategory".into(), table: "project_category".into(), child_key: "category_id".into() }));
    }

    #[test]
    fn commands_pipe_the_fields_and_name_the_tables() {
        let tables = [
            table("authors", vec![id(), column("name", "text", false), column("type", "int4", true)], &[]),
            table("book", vec![id(), column("author_id", "int8", false)], &[("author_id", "authors")]),
        ];
        let plan = plan(&tables, Dialect::Postgres);
        let commands = commands(&plan, Dialect::Postgres, Options { openapi: true, ..Options::default() });
        assert_eq!(
            commands[0],
            Command {
                args: "generate-model --name Author --table authors --fields --sqlx --postgres --openapi".split(' ').map(String::from).collect(),
                stdin: Some("name:String\ntype:Option<i32>\n\ny\n".into()),
            }
        );
        assert_eq!(
            commands[2].args.join(" "),
            "generate-relation --parent Author --child Book --name books --foreign-key author_id --parent-table authors --child-table book --sqlx --postgres --openapi"
        );
        let script = script(&commands, "octopux");
        assert!(script.contains("\noctopux generate-model --name Author --table authors --fields --sqlx --postgres --openapi <<'EOF'\nname:String\ntype:Option<i32>\n\ny\nEOF\n"));
    }
}
