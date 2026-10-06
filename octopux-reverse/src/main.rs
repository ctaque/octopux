// Reverse engineers a database into octopux models and relations: its schema is read,
mod plan;
// then piped to `octopux generate-model --fields` and `octopux generate-relation`
mod schema;

use plan::{Command, Options, Plan};
use schema::{Database, Dialect};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{self, Stdio};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(
    name = "octopux-reverse",
    about = "Generates the octopux models and relations of the tables of a database.\n\
             Prints the `octopux` commands as a shell script (`octopux-reverse | sh`), or runs them with --run."
)]
struct Cli {
    /// The database to read, `sqlite:`, `postgres:` or `mysql:`
    #[structopt(long = "database-url", env = "DATABASE_URL")]
    database_url: Option<String>,
    /// Applies the sqlx migrations of this folder before reading the tables, to an in-memory SQLite database
    /// without --database-url. The migrations are applied to the --database-url database: use a throwaway one
    #[structopt(long = "migrations", parse(from_os_str))]
    migrations: Option<PathBuf>,
    /// Reads the tables of this PostgreSQL schema instead of the current one (`public` by default),
    /// --migrations are then applied to it, requires a `postgres:` --database-url
    #[structopt(long = "schema")]
    schema: Option<String>,
    /// Only reads these tables, comma separated
    #[structopt(long = "tables", use_delimiter = true)]
    tables: Vec<String>,
    /// Leaves out these tables, comma separated
    #[structopt(long = "exclude", use_delimiter = true)]
    exclude: Vec<String>,
    /// Generates the models only
    #[structopt(long = "no-relations")]
    no_relations: bool,
    /// Passes --openapi to the octopux commands
    #[structopt(long = "openapi")]
    openapi: bool,
    /// Passes --graphql to the octopux commands
    #[structopt(long = "graphql")]
    graphql: bool,
    /// Passes --force to the octopux commands, overwriting the existing models and relations
    #[structopt(long = "force")]
    force: bool,
    /// Passes --output to the octopux commands, the folder of the generated models and relations
    #[structopt(long = "output")]
    output: Option<String>,
    /// Runs the octopux commands in the working directory instead of printing them
    #[structopt(long = "run")]
    run: bool,
    /// The octopux CLI
    #[structopt(long = "octopux", default_value = "octopux")]
    octopux: String,
}

fn main() {
    let cli = Cli::from_args();
    if let Err(error) = reverse(&cli) {
        eprintln!("error: {}", error);
        process::exit(1);
    }
}

// The url must be one of the database of the --sqlite, --postgres or --mysql flag
fn check_dialect(cli: &Cli) -> Result<(), String> {
    let db_uri = cli
        .database_url
        .as_deref()
        .ok_or_else(|| String::from("needs DATABASE_URL or --database-url"))?;

    if cli.schema.is_some() && Dialect::from_url(db_uri) != Some(Dialect::Postgres) {
        return Err(format!("--schema reads a PostgreSQL schema, the url is not a `postgres:` one: `{db_uri}`"));
    }

    match db_uri.split_once(':').map(|(scheme, _)| scheme) {
        Some("sqlite" | "postgres" | "postgresql" | "mysql") => Ok(()),
        Some(scheme) => Err(format!(
            "unsupported database scheme `{scheme}` (expected sqlite, postgres or mysql)"
        )),
        None => Err(format!("invalid DATABASE_URL: `{db_uri}`")),
    }
}

fn reverse(cli: &Cli) -> Result<(), String> {
    check_dialect(cli)?;
    if cli.database_url.is_none() && cli.migrations.is_none() {
        return Err("set DATABASE_URL or --database-url, or --migrations to read the tables they create".to_string());
    }
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())?;
    let (dialect, tables) = runtime.block_on(async {
        let mut db = Database::connect(cli.database_url.as_deref(), cli.migrations.is_some()).await?;
        if let Some(schema) = &cli.schema {
            db.use_schema(schema).await?;
        }
        if let Some(dir) = &cli.migrations {
            db.migrate(dir).await.map_err(|e| format!("migrations of {} not applied: {}", dir.display(), e))?;
        }
        Ok::<_, String>((db.dialect(), db.tables().await?))
    })?;
    let tables: Vec<_> = tables
        .into_iter()
        .filter(|t| (cli.tables.is_empty() || cli.tables.contains(&t.name)) && !cli.exclude.contains(&t.name))
        .collect();

    let mut plan = plan::plan(&tables, dialect);
    if cli.no_relations {
        plan.relations.clear();
    }
    report(&plan, dialect);
    if let Some(schema) = &cli.schema {
        eprintln!("note: the models query the tables without their schema, add `{}` to the search_path of the app", schema);
    }
    let options = Options { openapi: cli.openapi, graphql: cli.graphql, force: cli.force, output: cli.output.clone() };
    let commands = plan::commands(&plan, dialect, &options);
    if cli.run {
        commands.iter().try_for_each(|command| run(&cli.octopux, command))?;
        eprintln!("{}", plan::mounting(&plan));
    } else {
        print!("{}", plan::script(&commands, &cli.octopux));
        // shell comments, still a script
        println!("\n# {}", plan::mounting(&plan).replace('\n', "\n# "));
    }
    Ok(())
}

// What was read and left out, on stderr so that the script can be piped
fn report(plan: &Plan, dialect: Dialect) {
    for warning in &plan.warnings {
        eprintln!("warning: {}", warning);
    }
    let crates: Vec<&str> = [("uuid::", "uuid"), ("rust_decimal::", "rust_decimal"), ("serde_json::", "json")]
        .into_iter()
        .filter(|(path, _)| plan.models.iter().any(|m| m.fields.iter().any(|(_, ty)| ty.contains(path))))
        .map(|(_, feature)| feature)
        .collect();
    if !crates.is_empty() {
        eprintln!("note: the models use the `{}` sqlx features, and their crates", crates.join("`, `"));
    }
    eprintln!(
        "{} model{} and {} relation{} read from the {} database",
        plan.models.len(),
        if plan.models.len() != 1 { "s" } else { "" },
        plan.relations.len(),
        if plan.relations.len() != 1 { "s" } else { "" },
        dialect.name(),
    );
}

// Runs `octopux` with the arguments of `command`, its answers piped to the prompts
fn run(octopux: &str, command: &Command) -> Result<(), String> {
    eprintln!("$ {} {}", octopux, command.args.join(" "));
    let mut child = process::Command::new(octopux)
        .args(&command.args)
        .stdin(if command.stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .spawn()
        .map_err(|e| format!("{} not run ({}), install it with `cargo install octopux-cli` or pass --octopux", octopux, e))?;
    if let (Some(mut input), Some(stdin)) = (child.stdin.take(), &command.stdin) {
        // octopux exits before reading its prompts when the file exists without --force, and closes the pipe
        if let Err(e) = input.write_all(stdin.as_bytes()) {
            if e.kind() != io::ErrorKind::BrokenPipe {
                return Err(e.to_string());
            }
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`{} {}` failed ({})", octopux, command.args.join(" "), status))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(args: &str) -> Result<(), String> {
        check_dialect(&Cli::from_iter_safe(args.split(' ')).unwrap())
    }

    #[test]
    fn the_url_is_a_supported_one() {
        assert_eq!(check("octopux-reverse --database-url mysql://localhost/db"), Ok(()));
        assert_eq!(check("octopux-reverse --database-url postgres://localhost/db"), Ok(()));
        assert_eq!(check("octopux-reverse --database-url postgresql://localhost/db"), Ok(()));
        assert_eq!(check("octopux-reverse --database-url sqlite://app.db"), Ok(()));
        assert!(check("octopux-reverse --database-url mssql://localhost/db").is_err());
        assert!(check("octopux-reverse --database-url localhost").is_err());
        assert!(check("octopux-reverse").is_err());
        assert!(Cli::from_iter_safe(["octopux-reverse", "--mysql"]).is_err());
    }

    #[test]
    fn the_schema_requires_postgres() {
        assert_eq!(check("octopux-reverse --schema app --database-url postgres://localhost/db"), Ok(()));
        assert_eq!(check("octopux-reverse --schema app --database-url postgresql://localhost/db"), Ok(()));
        assert!(check("octopux-reverse --schema app --database-url mysql://localhost/db").is_err());
        assert!(check("octopux-reverse --schema app --database-url sqlite::memory:").is_err());
    }
}
