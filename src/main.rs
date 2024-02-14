use chrono::{TimeZone, Utc};
use comfy_table::Table;
use comfy_table::{presets::UTF8_FULL, ContentArrangement};
use git2::{Commit as GitCommit, Repository, Revwalk};
use rusqlite::{types::Value, Connection, Error as SqlError, Result};
use std::io::{stdin, stdout, Write};

fn insert_commit(conn: &Connection, commit: &GitCommit) -> Result<()> {
    let datetime = Utc.timestamp_opt(commit.time().seconds(), 0);
    conn.execute(
        "INSERT INTO commits (id, author, date, message) VALUES (?1, ?2, ?3, ?4)",
        [
            // only store first 7 chars of commit id
            commit.id().to_string().chars().take(7).collect(),
            commit.author().name().unwrap_or("None").to_string(),
            datetime.unwrap().to_string(),
            commit.message().unwrap_or("None").to_string(),
        ],
    )?;

    Ok(())
}

fn init_db(repo: &Repository, revwalk: Revwalk) -> Result<Connection, SqlError> {
    let conn = Connection::open_in_memory()?;

    conn.execute(
        "CREATE TABLE commits (
                        id       TEXT PRIMARY KEY,
                        author   TEXT NOT NULL,
                        date     TEXT NOT NULL,
                        message  TEXT NOT NULL
                    )",
        (),
    )?;

    for commit_id in revwalk {
        let commit_id = commit_id.expect("Failed to get commit ID");
        let commit = repo.find_commit(commit_id).expect("Failed to find commit");

        insert_commit(&conn, &commit)?;
    }

    Ok(conn)
}

fn value_to_string(value: Value) -> String {
    match value {
        Value::Integer(i) => i.to_string(),
        Value::Real(f) => f.to_string(),
        Value::Text(s) => s,
        Value::Blob(_) => String::from("Blob"),
        Value::Null => String::from("NULL"),
    }
}

fn run_sql_query(conn: &Connection, sql: &str) -> Result<(), SqlError> {
    let mut stmt = conn.prepare(sql)?;
    let column_names: Vec<&str> = stmt.column_names().into_iter().collect();
    let column_len = column_names.len();

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_width(80)
        .set_header(&column_names);

    let mut rows = stmt.query([])?;
    let mut row_count = 0;

    while let Some(row) = rows.next()? {
        let values: Vec<String> = (0..column_len)
            .map(|col_idx| {
                let value: Value = row.get(col_idx).unwrap_or(Value::Null);
                value_to_string(value)
            })
            .collect();

        table.add_row(values);
        row_count += 1;
    }

    println!("{table}");
    println!("Rows returned: {}", row_count);

    Ok(())
}

const TERMINAL_PROMPT: &str = ">> ";
const INIT_SQL_QUERY: &str = "SELECT * FROM COMMITS ORDER BY date DESC LIMIT 1;";

fn main() -> Result<(), String> {
    // TODO: take repo_path as option
    let repo_path = "./";

    let repo = Repository::open(repo_path).map_err(|err| format!("Cannot open repo. {}", err))?;

    // Create a revwalk to traverse the commit history
    let mut revwalk = repo.revwalk().expect("Failed to create revwalk");
    revwalk.push_head().expect("Failed to push HEAD OID");

    let conn = init_db(&repo, revwalk).map_err(|err| format!("DB error. {}", err))?;

    // Run initial SQL query
    println!("{}{}", TERMINAL_PROMPT, INIT_SQL_QUERY);
    run_sql_query(&conn, INIT_SQL_QUERY)
        .map_err(|err| format!("Initial SQL query failed. {}", err))?;

    loop {
        print!("{}", TERMINAL_PROMPT);
        // Ensure the prompt is displayed immediately
        stdout()
            .flush()
            .map_err(|err| format!("Flush error. {}", err))?;

        let mut input = String::new();
        stdin()
            .read_line(&mut input)
            .map_err(|err| format!("Failed to read line. {}", err))?;

        let input = input.trim(); // Remove newline characters

        match input {
            "exit" | "quit" => break,
            "help" => {
                println!("Available commands:");
                println!(" - `help`: Display this help message.");
                println!(" - `exit` or `quit`: Exit the program.");
                println!(" - Enter SQL at the prompt to see results.");
            }
            "" => {}
            _ => {
                if let Err(err) = run_sql_query(&conn, input) {
                    eprintln!("SQL error. {}", err);
                }
            }
        }
    }

    Ok(())
}
