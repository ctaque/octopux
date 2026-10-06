// The tables created by the migrations drawn in the console: a box per table with its columns and keys,
// laid out side by side over the console width, then the relations of their foreign keys.
// In a terminal, the tables are filtered as the user types a table, column or constraint name
use crate::{bold, cyan, dim, magenta, yellow, Column, Table};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::{cursor, execute, queue, terminal};
use std::io::{self, Write};

// Columns of a join table besides its two foreign keys
const JOIN_COLUMNS: [&str; 4] = ["id", "created_at", "updated_at", "deleted_at"];

// Spaces between two tables of a row, and empty lines between two rows
const GAP: usize = 4;
const ROW_GAP: usize = 2;

fn width(s: &str) -> usize {
    s.chars().count()
}

// The width the text takes in the console, without its color escapes
fn visible_width(s: &str) -> usize {
    let mut escape = false;
    s.chars()
        .filter(|&c| {
            match (escape, c) {
                (false, '\x1b') => escape = true,
                (true, 'm') => escape = false,
                (false, _) => return true,
                _ => {}
            }
            false
        })
        .count()
}

// The columns of the console: COLUMNS, then the size of the terminal, 80 when the output is not one
pub fn console_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|c| c.parse().ok())
        .or_else(|| terminal::size().ok().map(|(w, _)| w as usize))
        .unwrap_or(80)
}

pub fn render(tables: &[Table], console_width: usize) -> String {
    let boxes: Vec<String> = tables.iter().map(render_table).collect();
    let mut out = layout(&boxes, console_width);
    out += &render_relations(tables);
    out += &dim("PK primary key · FK foreign key · uniq unique · ? nullable");
    out.push('\n');
    out
}

fn key(column: &Column) -> &'static str {
    match (column.primary_key, column.references.is_some()) {
        (true, true) => "PK FK",
        (true, false) => "PK",
        (false, true) => "FK",
        (false, false) => "",
    }
}

// The `uniq` and `not null` attributes of a column that is not the primary key, which is both, and its default value
fn attributes(column: &Column) -> [String; 3] {
    let uniq = if column.unique && !column.primary_key { "uniq" } else { "" };
    let not_null = if !column.nullable && !column.primary_key { "not null" } else { "" };
    [uniq.to_string(), not_null.to_string(), column.default.as_ref().map_or(String::new(), |d| format!("default {}", d))]
}

// The colors of the attributes, the constraints standing out from the default value
const ATTRIBUTE_COLORS: [fn(&str) -> String; 3] = [magenta, magenta, dim];

// ┌─────────────────────────────────────────────────────┐
// │ book                                                │
// ├─────────────────────────────────────────────────────┤
// │ PK id        INTEGER                                │
// │    title     TEXT?    uniq                          │
// │    status    TEXT          not null default 'draft' │
// │ FK author_id INTEGER?                               │ ──▶ author.id
// └─────────────────────────────────────────────────────┘
fn render_table(table: &Table) -> String {
    let types: Vec<String> = table.columns.iter().map(|c| format!("{}{}", c.sql_type, if c.nullable { "?" } else { "" })).collect();
    let attributes: Vec<[String; 3]> = table.columns.iter().map(attributes).collect();
    let key_width = table.columns.iter().map(|c| width(key(c))).max().unwrap_or(0);
    let name_width = table.columns.iter().map(|c| width(&c.name)).max().unwrap_or(0);
    let type_width = types.iter().map(|t| width(t)).max().unwrap_or(0);
    // an attribute no column has takes no room
    let attribute_widths: Vec<usize> = (0..3).map(|i| attributes.iter().map(|a| width(&a[i])).max().unwrap_or(0)).collect();
    let column_width = |w: usize| if w > 0 { w + 1 } else { 0 };
    let row_width = column_width(key_width) + name_width + 1 + type_width + attribute_widths.iter().map(|w| column_width(*w)).sum::<usize>();
    let inner = row_width.max(width(&table.name));
    let line = "─".repeat(inner + 2);

    let mut out = format!("┌{}┐\n", line);
    out += &format!("│ {}{} │\n", bold(&table.name), " ".repeat(inner - width(&table.name)));
    out += &format!("├{}┤\n", line);
    for ((column, ty), values) in table.columns.iter().zip(&types).zip(&attributes) {
        let key = match key_width {
            0 => String::new(),
            _ if column.primary_key => format!("{} ", yellow(&format!("{:key_width$}", key(column)))),
            _ => format!("{} ", cyan(&format!("{:key_width$}", key(column)))),
        };
        let attributes: String = values
            .iter()
            .zip(&attribute_widths)
            .zip(ATTRIBUTE_COLORS)
            .filter(|((_, w), _)| **w > 0)
            .map(|((value, w), paint)| format!(" {}", paint(&format!("{:w$}", value))))
            .collect();
        let padding = " ".repeat(inner - row_width);
        out += &format!("│ {}{:name_width$} {}{}{} │", key, column.name, dim(&format!("{:type_width$}", ty)), attributes, padding);
        if let Some(reference) = &column.references {
            out += &format!(" {}", cyan(&format!("──▶ {}.{}", reference.table, reference.column)));
        }
        if !column.constraints.is_empty() {
            out += &format!(" {}", dim(&column.constraints.join(" ")));
        }
        out.push('\n');
    }
    out += &format!("└{}┘\n", line);
    out
}

// The boxes side by side, in their order, as many per row as the console width holds, aligned on their top line
fn layout(boxes: &[String], console_width: usize) -> String {
    let blocks: Vec<(Vec<&str>, usize)> = boxes
        .iter()
        .map(|b| {
            let lines: Vec<&str> = b.lines().collect();
            let width = lines.iter().map(|l| visible_width(l)).max().unwrap_or(0);
            (lines, width)
        })
        .collect();
    let mut rows: Vec<&[(Vec<&str>, usize)]> = Vec::new();
    let mut start = 0;
    let mut row_width = 0;
    for (i, (_, width)) in blocks.iter().enumerate() {
        if i > start && row_width + GAP + width > console_width {
            rows.push(&blocks[start..i]);
            start = i;
        }
        row_width = if i == start { *width } else { row_width + GAP + width };
    }
    if start < blocks.len() {
        rows.push(&blocks[start..]);
    }

    let mut out = String::new();
    for row in rows {
        let height = row.iter().map(|(lines, _)| lines.len()).max().unwrap_or(0);
        for i in 0..height {
            let mut line = String::new();
            for (n, (lines, width)) in row.iter().enumerate() {
                let text = lines.get(i).copied().unwrap_or_default();
                if n > 0 {
                    line += &" ".repeat(GAP);
                }
                line += text;
                line += &" ".repeat(width - visible_width(text));
            }
            out += line.trim_end();
            out.push('\n');
        }
        out += &"\n".repeat(ROW_GAP);
    }
    out
}

// The keywords of the typed text, separated by commas or pipes: `country, city` or `country|city`
fn keywords(typed: &str) -> Vec<String> {
    typed.split([',', '|']).map(|k| k.trim().to_lowercase()).filter(|k| !k.is_empty()).collect()
}

fn contains(text: &str, keywords: &[String]) -> bool {
    let text = text.to_lowercase();
    keywords.iter().any(|k| text.contains(k.as_str()))
}

// The name, the constraint names and the keys of a column, a `UNIQUE` or `PK FK` column matches `unique` or `fk`
fn column_matches(column: &Column, keywords: &[String]) -> bool {
    let unique = if column.unique && !column.primary_key { "UNIQUE" } else { "" };
    [column.name.as_str(), key(column), unique]
        .into_iter()
        .chain(column.constraints.iter().map(String::as_str))
        .any(|c| !c.is_empty() && contains(c, keywords))
}

// The tables whose name contains one of the typed keywords, whole, and the other tables with only their columns
// whose name or constraint contains one, a table left without columns is dropped
pub fn filter(tables: &[Table], typed: &str) -> Vec<Table> {
    let keywords = keywords(typed);
    if keywords.is_empty() {
        return tables.to_vec();
    }
    tables
        .iter()
        .filter_map(|t| {
            if contains(&t.name, &keywords) {
                return Some(t.clone());
            }
            let columns: Vec<Column> = t.columns.iter().filter(|c| column_matches(c, &keywords)).cloned().collect();
            (!columns.is_empty()).then(|| Table { name: t.name.clone(), columns })
        })
        .collect()
}

const PROMPT: &str = "Filter";

// Lines of the header: the framed filter input, the status line and an empty line
const HEADER: usize = 5;

// ╭─ Filter ─────────────────────────────╮
// │ › country                            │
// ╰──────────────────────────────────────╯
//   5/100 tables · …
fn render_header(typed: &str, status: &str, columns: usize) -> String {
    let inner = columns.saturating_sub(2).max(width(PROMPT) + 3);
    let title = format!("─ {} ", PROMPT);
    let mut out = format!("{}{}{}\r\n", cyan("╭"), bold(&cyan(&title)), cyan(&format!("{}╮", "─".repeat(inner - width(&title)))));
    let input = format!(" {} {}", cyan("›"), bold(typed));
    out += &format!("{}{}{}{}\r\n", cyan("│"), input, " ".repeat(inner.saturating_sub(3 + width(typed))), cyan("│"));
    out += &format!("{}\r\n", cyan(&format!("╰{}╯", "─".repeat(inner))));
    out += &format!("  {}\r\n\r\n", dim(status));
    out
}

// The filter input above the drawn tables, matching the table, column and constraint names:
// the arrows and the pages scroll, Esc quits
pub fn explore(tables: &[Table]) -> io::Result<()> {
    let mut out = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(out, terminal::EnterAlternateScreen, terminal::DisableLineWrap)?;
    let result = explore_loop(&mut out, tables);
    execute!(out, terminal::EnableLineWrap, terminal::LeaveAlternateScreen, cursor::Show)?;
    terminal::disable_raw_mode()?;
    result
}

fn explore_loop(out: &mut impl Write, tables: &[Table]) -> io::Result<()> {
    let mut typed = String::new();
    let mut scroll = 0;
    loop {
        let (columns, rows) = terminal::size()?;
        let shown = filter(tables, &typed);
        let drawing = if shown.is_empty() { dim("No table, column or constraint matches the filter\n") } else { render(&shown, columns as usize) };
        let lines: Vec<&str> = drawing.lines().collect();
        let page = (rows as usize).saturating_sub(HEADER).max(1);
        scroll = scroll.min(lines.len().saturating_sub(page));

        queue!(out, cursor::Hide, cursor::MoveTo(0, 0), terminal::Clear(terminal::ClearType::All))?;
        let status = format!("{}/{} tables · table, column or constraint names, separated by , or | · ↑↓ PgUp PgDn scroll · Esc quit", shown.len(), tables.len());
        write!(out, "{}", render_header(&typed, &status, columns as usize))?;
        // no line break after the last line, which would scroll the screen up and the input out of the cursor
        let body: Vec<&str> = lines.iter().skip(scroll).take(page).copied().collect();
        write!(out, "{}", body.join("\r\n"))?;
        queue!(out, cursor::MoveTo((4 + width(&typed)) as u16, 1), cursor::Show)?;
        out.flush()?;

        let Event::Key(KeyEvent { code, modifiers, kind: KeyEventKind::Press | KeyEventKind::Repeat, .. }) = event::read()? else { continue };
        match code {
            KeyCode::Esc => return Ok(()),
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => return Ok(()),
            KeyCode::Up => scroll = scroll.saturating_sub(1),
            KeyCode::Down => scroll += 1,
            KeyCode::PageUp => scroll = scroll.saturating_sub(page),
            KeyCode::PageDown => scroll += page,
            KeyCode::Backspace => {
                typed.pop();
                scroll = 0;
            }
            KeyCode::Char(c) => {
                typed.push(c);
                scroll = 0;
            }
            _ => {}
        }
    }
}

// A join table holds two foreign keys to other tables, besides an `id` and the timestamps
fn join_keys(table: &Table) -> Option<[&Column; 2]> {
    let keys: Vec<&Column> = table.columns.iter().filter(|c| c.references.is_some()).collect();
    let others_only = table.columns.iter().all(|c| c.references.is_some() || JOIN_COLUMNS.contains(&c.name.as_str()));
    match keys.as_slice() {
        [a, b] if others_only => Some([*a, *b]),
        _ => None,
    }
}

// author  1──N  book      book.author_id
// book    N──N  category  book_category
// a unique foreign key is one-to-one
fn render_relations(tables: &[Table]) -> String {
    let mut rows: Vec<(String, &str, String, String)> = Vec::new();
    for table in tables {
        if let Some([a, b]) = join_keys(table) {
            let (a, b) = (a.references.as_ref().unwrap(), b.references.as_ref().unwrap());
            rows.push((a.table.clone(), "N──N", b.table.clone(), table.name.clone()));
            continue;
        }
        for column in &table.columns {
            if let Some(reference) = &column.references {
                let cardinality = if column.unique { "1──1" } else { "1──N" };
                rows.push((reference.table.clone(), cardinality, table.name.clone(), format!("{}.{}", table.name, column.name)));
            }
        }
    }
    if rows.is_empty() {
        return String::new();
    }
    let parent_width = rows.iter().map(|(p, _, _, _)| width(p)).max().unwrap_or(0);
    let child_width = rows.iter().map(|(_, _, c, _)| width(c)).max().unwrap_or(0);
    let mut out = format!("{}\n", bold("Relations"));
    for (parent, cardinality, child, via) in rows {
        out += &format!("  {:parent_width$}  {}  {:child_width$}  {}\n", parent, cyan(cardinality), child, dim(&via));
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_tables;

    #[test]
    fn tables_are_boxes_with_their_keys() {
        let tables = parse_tables(
            "CREATE TABLE book (id INTEGER PRIMARY KEY, title TEXT NOT NULL, author_id INTEGER REFERENCES author (id));
CREATE TABLE project_tag_assignment (id INTEGER PRIMARY KEY);",
        );
        assert_eq!(
            render_table(&tables[0]),
            "┌────────────────────────────────┐
│ book                           │
├────────────────────────────────┤
│ PK id        INTEGER           │
│    title     TEXT     not null │
│ FK author_id INTEGER?          │ ──▶ author.id
└────────────────────────────────┘
"
        );
        // a long name widens the box
        assert_eq!(
            render_table(&tables[1]),
            "┌────────────────────────┐
│ project_tag_assignment │
├────────────────────────┤
│ PK id INTEGER          │
└────────────────────────┘
"
        );
    }

    #[test]
    fn boxes_fill_the_console_width() {
        let boxes = ["┌──┐\n│a │\n└──┘\n", "┌───┐\n│b  │\n│c  │ ──▶ d.id\n└───┘\n", "┌─┐\n└─┘\n"].map(String::from);
        // the reference after a box counts in its width, the last box goes to the next row
        assert_eq!(
            layout(&boxes, 22),
            "┌──┐    ┌───┐
│a │    │b  │
└──┘    │c  │ ──▶ d.id
        └───┘


┌─┐
└─┘


"
        );
        // a box wider than the console is alone on its row
        assert_eq!(layout(&boxes[..2], 4), "┌──┐\n│a │\n└──┘\n\n\n┌───┐\n│b  │\n│c  │ ──▶ d.id\n└───┘\n\n\n");
        assert_eq!(visible_width(&format!("\x1b[1m{}\x1b[0m", "book")), 4);
    }

    #[test]
    fn the_typed_filter_keeps_the_matching_tables_and_columns() {
        let tables = parse_tables(
            "CREATE TABLE author (id INT8 PRIMARY KEY, name TEXT NOT NULL, email TEXT CONSTRAINT author_email_key UNIQUE);
CREATE TABLE book (id INT8 PRIMARY KEY, title TEXT, author_id INT8, CONSTRAINT fk_book_author_id FOREIGN KEY (author_id) REFERENCES author (id));",
        );
        let names = |typed: &str| -> Vec<String> {
            super::filter(&tables, typed).iter().map(|t| format!("{}({})", t.name, t.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(","))).collect()
        };
        assert_eq!(names(" "), ["author(id,name,email)", "book(id,title,author_id)"]);
        // a matching table is drawn whole, the other tables with their matching columns
        assert_eq!(names("AUTHOR"), ["author(id,name,email)", "book(author_id)"]);
        assert_eq!(names("title"), ["book(title)"]);
        // by constraint name, or by key
        assert_eq!(names("fk_book"), ["book(author_id)"]);
        assert_eq!(names("unique"), ["author(email)"]);
        assert_eq!(names("pk"), ["author(id)", "book(id)"]);
        assert!(names("nope").is_empty());
        // the keywords separated by commas or pipes add up
        assert_eq!(names("title, unique"), ["author(email)", "book(title)"]);
        assert_eq!(names("nope|BOOK|"), ["book(id,title,author_id)"]);
        assert_eq!(names(" , |"), names(""));
        // the names follow the columns they constrain
        assert!(render_table(&tables[1]).contains("──▶ author.id fk_book_author_id"));
    }

    #[test]
    fn unique_not_null_columns_and_defaults_are_attributes() {
        let tables = parse_tables(
            "CREATE TABLE book (id INTEGER PRIMARY KEY, title TEXT UNIQUE, status TEXT NOT NULL DEFAULT 'draft', author_id INTEGER REFERENCES author (id));",
        );
        assert_eq!(
            render_table(&tables[0]),
            "┌─────────────────────────────────────────────────────┐
│ book                                                │
├─────────────────────────────────────────────────────┤
│ PK id        INTEGER                                │
│    title     TEXT?    uniq                          │
│    status    TEXT          not null default 'draft' │
│ FK author_id INTEGER?                               │ ──▶ author.id
└─────────────────────────────────────────────────────┘
"
        );
    }

    #[test]
    fn foreign_keys_give_the_relations() {
        let tables = parse_tables(
            "CREATE TABLE author (id INT8 PRIMARY KEY, name TEXT NOT NULL);
CREATE TABLE profile (id INT8 PRIMARY KEY, author_id INT8 NOT NULL UNIQUE REFERENCES author (id));
CREATE TABLE book (id INT8 PRIMARY KEY, author_id INT8 NOT NULL, CONSTRAINT fk_book_author_id FOREIGN KEY (author_id) REFERENCES author (id));
CREATE TABLE category (id INT8 PRIMARY KEY, label TEXT NOT NULL);
CREATE TABLE book_category (book_id INT8 REFERENCES book (id), category_id INT8 REFERENCES category, PRIMARY KEY (book_id, category_id));",
        );
        assert_eq!(
            render_relations(&tables),
            "Relations
  author  1──1  profile   profile.author_id
  author  1──N  book      book.author_id
  book    N──N  category  book_category

"
        );
    }
}
