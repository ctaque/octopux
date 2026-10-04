// The schema of a database: its tables, their columns, primary keys and single column foreign keys
use sqlx::Connection;
use std::collections::BTreeMap;
use std::path::Path;

// Database of a connection url, from its scheme
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dialect {
    Sqlite,
    Postgres,
    Mysql,
}

impl Dialect {
    pub fn from_url(url: &str) -> Option<Dialect> {
        match url.split(':').next()? {
            "sqlite" => Some(Dialect::Sqlite),
            "postgres" | "postgresql" => Some(Dialect::Postgres),
            "mysql" | "mariadb" => Some(Dialect::Mysql),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Dialect::Sqlite => "SQLite",
            Dialect::Postgres => "PostgreSQL",
            Dialect::Mysql => "MySQL",
        }
    }

    // The flag of the octopux CLI targeting the database
    pub fn flag(self) -> &'static str {
        match self {
            Dialect::Sqlite => "--sqlite",
            Dialect::Postgres => "--postgres",
            Dialect::Mysql => "--mysql",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub name: String,
    // as the database declares it: `INTEGER` (SQLite), `int8` (the PostgreSQL udt name), `int unsigned` (MySQL)
    pub sql_type: String,
    pub nullable: bool,
    pub primary_key: bool,
}

// A single column foreign key, `column` referencing `references` of `table`
#[derive(Debug, Clone, PartialEq)]
pub struct ForeignKey {
    pub column: String,
    pub table: String,
    pub references: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    pub foreign_keys: Vec<ForeignKey>,
}

impl Table {
    pub fn column(&self, name: &str) -> Option<&Column> {
        self.columns.iter().find(|c| c.name == name)
    }
}

// Table of the sqlx migrations, not a model
const SQLX_MIGRATIONS_TABLE: &str = "_sqlx_migrations";

const POSTGRES_COLUMNS: &str = "SELECT c.table_name::text, c.column_name::text, c.udt_name::text, c.is_nullable = 'YES'
FROM information_schema.columns c
JOIN information_schema.tables t ON t.table_schema = c.table_schema AND t.table_name = c.table_name
WHERE c.table_schema = current_schema() AND t.table_type = 'BASE TABLE' AND c.table_name <> '_sqlx_migrations'
ORDER BY c.table_name, c.ordinal_position";

const POSTGRES_PRIMARY_KEYS: &str = "SELECT cl.relname::text, a.attname::text
FROM pg_constraint c
JOIN pg_class cl ON cl.oid = c.conrelid
JOIN pg_namespace n ON n.oid = cl.relnamespace
JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = ANY (c.conkey)
WHERE c.contype = 'p' AND n.nspname = current_schema()";

// the foreign keys of several columns are left out, a relation joins on one column
const POSTGRES_FOREIGN_KEYS: &str = "SELECT cl.relname::text, a.attname::text, rcl.relname::text, ra.attname::text
FROM pg_constraint c
JOIN pg_class cl ON cl.oid = c.conrelid
JOIN pg_namespace n ON n.oid = cl.relnamespace
JOIN pg_class rcl ON rcl.oid = c.confrelid
JOIN pg_namespace rn ON rn.oid = rcl.relnamespace
JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = c.conkey[1]
JOIN pg_attribute ra ON ra.attrelid = c.confrelid AND ra.attnum = c.confkey[1]
WHERE c.contype = 'f' AND n.nspname = current_schema() AND rn.nspname = current_schema() AND array_length(c.conkey, 1) = 1
ORDER BY 1, 2";

// information_schema columns are cast, MySQL 8 returns some of them as binary strings.
// The primary key is the `PRIMARY` constraint: COLUMN_KEY is also `PRI` for the first unique NOT NULL index
// of a table without primary key
const MYSQL_COLUMNS: &str = "SELECT CAST(c.TABLE_NAME AS CHAR), CAST(c.COLUMN_NAME AS CHAR), CAST(c.COLUMN_TYPE AS CHAR),
    CAST(c.IS_NULLABLE = 'YES' AS SIGNED),
    CAST(EXISTS (SELECT 1 FROM information_schema.KEY_COLUMN_USAGE k
        WHERE k.TABLE_SCHEMA = c.TABLE_SCHEMA AND k.TABLE_NAME = c.TABLE_NAME AND k.COLUMN_NAME = c.COLUMN_NAME
        AND k.CONSTRAINT_NAME = 'PRIMARY') AS SIGNED)
FROM information_schema.COLUMNS c
JOIN information_schema.TABLES t ON t.TABLE_SCHEMA = c.TABLE_SCHEMA AND t.TABLE_NAME = c.TABLE_NAME
WHERE c.TABLE_SCHEMA = DATABASE() AND t.TABLE_TYPE = 'BASE TABLE' AND c.TABLE_NAME <> '_sqlx_migrations'
ORDER BY c.TABLE_NAME, c.ORDINAL_POSITION";

const MYSQL_FOREIGN_KEYS: &str = "SELECT CAST(k.TABLE_NAME AS CHAR), CAST(k.CONSTRAINT_NAME AS CHAR), CAST(k.COLUMN_NAME AS CHAR),
    CAST(k.REFERENCED_TABLE_NAME AS CHAR), CAST(k.REFERENCED_COLUMN_NAME AS CHAR)
FROM information_schema.KEY_COLUMN_USAGE k
WHERE k.TABLE_SCHEMA = DATABASE() AND k.REFERENCED_TABLE_SCHEMA = DATABASE() AND k.REFERENCED_TABLE_NAME IS NOT NULL
ORDER BY k.TABLE_NAME, k.CONSTRAINT_NAME, k.ORDINAL_POSITION";

// Groups the (table, column) rows, ordered by table, into tables without foreign keys
fn group_columns(rows: impl IntoIterator<Item = (String, Column)>) -> Vec<Table> {
    let mut tables: Vec<Table> = Vec::new();
    for (table, column) in rows {
        match tables.last_mut() {
            Some(last) if last.name == table => last.columns.push(column),
            _ => tables.push(Table { name: table, columns: vec![column], foreign_keys: Vec::new() }),
        }
    }
    tables
}

// Adds the (table, constraint, column, referenced table, referenced column) rows to the tables,
// leaving out the constraints of several columns
fn add_foreign_keys(tables: &mut [Table], rows: impl IntoIterator<Item = (String, String, String, String, String)>) {
    let mut constraints: BTreeMap<(String, String), Vec<ForeignKey>> = BTreeMap::new();
    for (table, constraint, column, references_table, references) in rows {
        constraints.entry((table, constraint)).or_default().push(ForeignKey { column, table: references_table, references });
    }
    for ((table, _), mut keys) in constraints {
        if keys.len() != 1 {
            continue;
        }
        if let Some(t) = tables.iter_mut().find(|t| t.name == table) {
            t.foreign_keys.push(keys.remove(0));
        }
    }
}

// An open connection to the database of a url
pub enum Database {
    Sqlite(sqlx::SqliteConnection),
    Postgres(sqlx::PgConnection),
    Mysql(sqlx::MySqlConnection),
}

impl Database {
    // Connects to `url`, a SQLite file is opened read only unless `writable` (to apply migrations),
    // without a url, an in-memory SQLite database
    pub async fn connect(url: Option<&str>, writable: bool) -> Result<Database, String> {
        let error = |e: sqlx::Error| e.to_string();
        let Some(url) = url else {
            return Ok(Database::Sqlite(sqlx::SqliteConnection::connect("sqlite::memory:").await.map_err(error)?));
        };
        match Dialect::from_url(url) {
            Some(Dialect::Sqlite) => {
                use std::str::FromStr;
                let options = sqlx::sqlite::SqliteConnectOptions::from_str(url).map_err(error)?;
                let options = if writable { options.create_if_missing(true) } else { options.read_only(true).create_if_missing(false) };
                Ok(Database::Sqlite(sqlx::SqliteConnection::connect_with(&options).await.map_err(error)?))
            }
            Some(Dialect::Postgres) => Ok(Database::Postgres(sqlx::PgConnection::connect(url).await.map_err(error)?)),
            Some(Dialect::Mysql) => Ok(Database::Mysql(sqlx::MySqlConnection::connect(url).await.map_err(error)?)),
            None => Err("unsupported scheme, use sqlite:, postgres: or mysql:".to_string()),
        }
    }

    pub fn dialect(&self) -> Dialect {
        match self {
            Database::Sqlite(_) => Dialect::Sqlite,
            Database::Postgres(_) => Dialect::Postgres,
            Database::Mysql(_) => Dialect::Mysql,
        }
    }

    // Applies the sqlx migrations of `dir`
    pub async fn migrate(&mut self, dir: &Path) -> Result<(), String> {
        let migrator = sqlx::migrate::Migrator::new(dir).await.map_err(|e| e.to_string())?;
        match self {
            Database::Sqlite(conn) => migrator.run(conn).await,
            Database::Postgres(conn) => migrator.run(conn).await,
            Database::Mysql(conn) => migrator.run(conn).await,
        }
        .map_err(|e| e.to_string())
    }

    // The tables of the database, ordered by name
    pub async fn tables(&mut self) -> Result<Vec<Table>, String> {
        let error = |e: sqlx::Error| e.to_string();
        match self {
            Database::Sqlite(conn) => {
                let names: Vec<String> = sqlx::query_scalar(
                    "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name <> $1 ORDER BY name",
                )
                .bind(SQLX_MIGRATIONS_TABLE)
                .fetch_all(&mut *conn)
                .await
                .map_err(error)?;
                let mut tables = Vec::new();
                for name in names {
                    let columns: Vec<(String, String, i64, i64)> =
                        sqlx::query_as("SELECT name, type, \"notnull\", pk FROM pragma_table_info($1) ORDER BY cid")
                            .bind(&name)
                            .fetch_all(&mut *conn)
                            .await
                            .map_err(error)?;
                    // `to` is NULL when the key references the primary key of the table
                    let keys: Vec<(i64, String, String, Option<String>)> =
                        sqlx::query_as("SELECT id, \"from\", \"table\", \"to\" FROM pragma_foreign_key_list($1) ORDER BY id, seq")
                            .bind(&name)
                            .fetch_all(&mut *conn)
                            .await
                            .map_err(error)?;
                    let without_rowid: bool = sqlx::query_scalar("SELECT wr FROM pragma_table_list($1) WHERE schema = 'main'")
                        .bind(&name)
                        .fetch_one(&mut *conn)
                        .await
                        .map_err(error)?;
                    // a primary key column holds NULL, unless it is NOT NULL, the INTEGER alias of the rowid
                    // or in a WITHOUT ROWID table (https://sqlite.org/lang_createtable.html#the_primary_key)
                    let rowid_alias = columns.iter().filter(|(_, _, _, pk)| *pk > 0).count() == 1;
                    let mut table = Table {
                        name: name.clone(),
                        columns: columns
                            .into_iter()
                            .map(|(name, sql_type, not_null, pk)| {
                                let not_null = not_null != 0 || (pk > 0 && (without_rowid || (rowid_alias && sql_type.eq_ignore_ascii_case("INTEGER"))));
                                Column { name, sql_type, nullable: !not_null, primary_key: pk > 0 }
                            })
                            .collect(),
                        foreign_keys: Vec::new(),
                    };
                    let rows = keys.into_iter().map(|(id, column, references_table, references)| {
                        (name.clone(), id.to_string(), column, references_table, references.unwrap_or_else(|| "id".to_string()))
                    });
                    add_foreign_keys(std::slice::from_mut(&mut table), rows);
                    tables.push(table);
                }
                Ok(tables)
            }
            Database::Postgres(conn) => {
                let columns: Vec<(String, String, String, bool)> = sqlx::query_as(POSTGRES_COLUMNS).fetch_all(&mut *conn).await.map_err(error)?;
                let primary_keys: Vec<(String, String)> = sqlx::query_as(POSTGRES_PRIMARY_KEYS).fetch_all(&mut *conn).await.map_err(error)?;
                let keys: Vec<(String, String, String, String)> = sqlx::query_as(POSTGRES_FOREIGN_KEYS).fetch_all(&mut *conn).await.map_err(error)?;
                let mut tables = group_columns(columns.into_iter().map(|(table, name, udt, nullable)| {
                    let primary_key = primary_keys.iter().any(|(t, c)| *t == table && *c == name);
                    (table, Column { name, sql_type: udt, nullable, primary_key })
                }));
                // the query already left out the keys of several columns
                let rows = keys.into_iter().map(|(table, column, rt, rc)| (table.clone(), column.clone(), column, rt, rc));
                add_foreign_keys(&mut tables, rows);
                Ok(tables)
            }
            Database::Mysql(conn) => {
                let columns: Vec<(String, String, String, i64, i64)> = sqlx::query_as(MYSQL_COLUMNS).fetch_all(&mut *conn).await.map_err(error)?;
                let keys: Vec<(String, String, String, String, String)> = sqlx::query_as(MYSQL_FOREIGN_KEYS).fetch_all(&mut *conn).await.map_err(error)?;
                let mut tables = group_columns(columns.into_iter().map(|(table, name, ty, nullable, pk)| {
                    (table, Column { name, sql_type: ty.to_lowercase(), nullable: nullable != 0, primary_key: pk != 0 })
                }));
                add_foreign_keys(&mut tables, keys);
                Ok(tables)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(future)
    }

    #[test]
    fn dialect_is_read_from_the_url_scheme() {
        assert_eq!(Dialect::from_url("sqlite://data.db"), Some(Dialect::Sqlite));
        assert_eq!(Dialect::from_url("postgresql://localhost/db"), Some(Dialect::Postgres));
        assert_eq!(Dialect::from_url("mariadb://localhost/db"), Some(Dialect::Mysql));
        assert_eq!(Dialect::from_url("redis://localhost"), None);
    }

    #[test]
    fn sqlite_tables_have_their_columns_and_foreign_keys() {
        let tables = block_on(async {
            let mut db = Database::connect(None, true).await.unwrap();
            let Database::Sqlite(conn) = &mut db else { unreachable!() };
            sqlx::raw_sql(
                "CREATE TABLE author (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL);
                CREATE TABLE book (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    title TEXT NOT NULL,
                    author_id INTEGER REFERENCES author,
                    a INTEGER, b INTEGER,
                    FOREIGN KEY (a, b) REFERENCES pair (x, y)
                );
                CREATE TABLE tag (id BIGINT PRIMARY KEY, name TEXT);
                CREATE TABLE label (id BIGINT PRIMARY KEY, name TEXT) WITHOUT ROWID;",
            )
            .execute(&mut *conn)
            .await
            .unwrap();
            db.tables().await.unwrap()
        });
        assert_eq!(tables.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["author", "book", "label", "tag"]);
        // only an INTEGER primary key aliases the rowid, a BIGINT one holds NULL outside a WITHOUT ROWID table
        assert_eq!(tables[2].column("id").map(|c| c.nullable), Some(false));
        assert_eq!(tables[3].column("id").map(|c| c.nullable), Some(true));
        let book = &tables[1];
        assert_eq!(book.column("id"), Some(&Column { name: "id".into(), sql_type: "INTEGER".into(), nullable: false, primary_key: true }));
        assert_eq!(book.column("title").map(|c| c.nullable), Some(false));
        assert_eq!(book.column("author_id").map(|c| c.nullable), Some(true));
        // the key of two columns is left out, the implicit referenced column is the primary key
        assert_eq!(book.foreign_keys, [ForeignKey { column: "author_id".into(), table: "author".into(), references: "id".into() }]);
    }

    // Against a throwaway MySQL database: OCTOPUX_REVERSE_MYSQL_URL=mysql://root:root@127.0.0.1/test cargo test
    #[test]
    fn mysql_tables_have_their_primary_keys_and_decode_into_their_field_types() {
        let Ok(url) = std::env::var("OCTOPUX_REVERSE_MYSQL_URL") else { return };
        let (tables, row) = block_on(async {
            let mut db = Database::connect(Some(&url), true).await.unwrap();
            let Database::Mysql(conn) = &mut db else { panic!("not a mysql: url") };
            sqlx::raw_sql(
                "DROP TABLE IF EXISTS reverse_book, reverse_setting;
                CREATE TABLE reverse_setting (id BIGINT NOT NULL UNIQUE, value TEXT);
                CREATE TABLE reverse_book (
                    id BIGINT AUTO_INCREMENT PRIMARY KEY, published BIT(1) NOT NULL, flags BIT(8) NOT NULL,
                    year YEAR NOT NULL, active BOOLEAN NOT NULL
                );
                INSERT INTO reverse_book (published, flags, year, active) VALUES (1, 5, 2024, TRUE);",
            )
            .execute(&mut *conn)
            .await
            .unwrap();
            let row: (bool, u64, u16, bool) =
                sqlx::query_as("SELECT published, flags, year, active FROM reverse_book").fetch_one(&mut *conn).await.unwrap();
            let tables = db.tables().await.unwrap();
            let Database::Mysql(conn) = &mut db else { unreachable!() };
            sqlx::raw_sql("DROP TABLE reverse_book, reverse_setting").execute(&mut *conn).await.unwrap();
            (tables, row)
        });
        assert_eq!(row, (true, 5, 2024, true));
        let setting = tables.iter().find(|t| t.name == "reverse_setting").unwrap();
        // a unique NOT NULL column is not the primary key
        assert_eq!(setting.column("id").map(|c| c.primary_key), Some(false));
        let book = tables.iter().find(|t| t.name == "reverse_book").unwrap();
        let types: Vec<_> = book.columns.iter().map(|c| crate::plan::rust_type(Dialect::Mysql, &c.sql_type).unwrap()).collect();
        assert_eq!(types, ["i64", "bool", "u64", "u16", "bool"]);
        assert!(book.column("id").unwrap().primary_key);
    }
}
