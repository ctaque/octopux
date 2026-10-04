// Reverse engineers a database into octopux models and relations: its schema is read,
// then piped to `octopux generate-model --fields` and `octopux generate-relation`
mod plan;
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
    /// Reads a SQLite database, `--database-url` is a `sqlite:` one
    #[structopt(long = "sqlite", conflicts_with_all = &["postgres", "mysql"])]
    sqlite: bool,
    /// Reads a PostgreSQL database, `--database-url` is a `postgres:` one
    #[structopt(long = "postgres", conflicts_with = "mysql")]
    postgres: bool,
    /// Reads a MySQL (or MariaDB) database, `--database-url` is a `mysql:` one.
    /// --migrations are then applied to it, there is no in-memory MySQL database
    #[structopt(long = "mysql")]
    mysql: bool,
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
    let expected = match (cli.sqlite, cli.postgres, cli.mysql) {
        (true, _, _) => Dialect::Sqlite,
        (_, true, _) => Dialect::Postgres,
        (_, _, true) => Dialect::Mysql,
        _ => return Ok(()),
    };
    match cli.database_url.as_deref() {
        Some(url) if Dialect::from_url(url) != Some(expected) => {
            let scheme = url.split(':').next().unwrap_or_default();
            Err(format!("{} reads a {} database, the url is a `{}:` one", expected.flag(), expected.name(), scheme))
        }
        // the migrations of a server database are not applied to the in-memory SQLite one
        None if expected != Dialect::Sqlite => Err(format!(
            "{} needs DATABASE_URL or --database-url{}",
            expected.flag(),
            if cli.migrations.is_some() { ", a throwaway database to apply the migrations to" } else { "" }
        )),
        _ => Ok(()),
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
    let options = Options { openapi: cli.openapi, graphql: cli.graphql, force: cli.force };
    let commands = plan::commands(&plan, dialect, options);
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
    fn the_url_is_one_of_the_database_flag() {
        assert_eq!(check("octopux-reverse --mysql --database-url mysql://localhost/db"), Ok(()));
        assert_eq!(check("octopux-reverse --mysql --database-url mariadb://localhost/db"), Ok(()));
        assert_eq!(check("octopux-reverse --database-url postgres://localhost/db"), Ok(()));
        assert_eq!(check("octopux-reverse --sqlite --migrations migrations"), Ok(()));
        assert_eq!(
            check("octopux-reverse --mysql --database-url postgres://localhost/db"),
            Err("--mysql reads a MySQL database, the url is a `postgres:` one".to_string())
        );
        assert_eq!(
            check("octopux-reverse --mysql --migrations migrations"),
            Err("--mysql needs DATABASE_URL or --database-url, a throwaway database to apply the migrations to".to_string())
        );
        assert!(Cli::from_iter_safe(["octopux-reverse", "--mysql", "--postgres"]).is_err());
    }
}
