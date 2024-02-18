use chrono::{TimeZone, Utc};
use comfy_table::Table;
use comfy_table::{presets::UTF8_FULL, ContentArrangement};
use git2::{
    Commit as GitCommit, Error as GitError, ObjectType, Oid, Repository, Revwalk, Signature,
};
use rusqlite::{types::Value, Connection, Error as SqlError, Result};
use std::io::{stdin, stdout, Write};

// Struct to support both annotated and lightweight git tags
struct GitTag<'a> {
    pub id: Oid,
    pub name: Option<&'a str>,
    pub target_id: Oid,
    pub target_type: Option<ObjectType>,
    pub tagger: Option<Signature<'a>>,
    pub message: Option<&'a str>,
}

// Function to insert a Git commit into the SQLite database
fn insert_commit(conn: &Connection, commit: &GitCommit) -> Result<()> {
    // Extract the commit datetime in UTC
    let datetime = Utc.timestamp_opt(commit.time().seconds(), 0);

    // Execute the SQL INSERT statement
    conn.execute(
        "INSERT INTO commits (id, author, date, message) VALUES (?1, ?2, ?3, ?4)",
        [
            // Store only the first 7 characters of the commit id
            commit.id().to_string().chars().take(7).collect(),
            commit.author().name().unwrap_or("None").to_string(),
            datetime.unwrap().to_string(),
            commit.message().unwrap_or("None").to_string(),
        ],
    )?;

    Ok(())
}

// Function to insert a Git tag into the SQLite database
fn insert_tag(conn: &Connection, tag: &GitTag) -> Result<()> {
    let target_type = match tag.target_type {
        Some(t) => t.to_string(),
        _ => String::from("None"),
    };

    let tagger = match &tag.tagger {
        Some(t) => t.name().unwrap_or("None").to_string(),
        _ => String::from("None"),
    };

    // Execute the SQL INSERT statement
    conn.execute(
        "INSERT INTO tags (id, name, target_id, target_type, tagger, message) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        [
            // Store only the first 7 characters of the tag id
            tag.id.to_string().chars().take(7).collect(),
            tag.name.unwrap_or("None").to_string(),
            // Store only the first 7 characters of the tag target id
            tag.target_id.to_string().chars().take(7).collect(),
            target_type,
            tagger,
            tag.message.unwrap_or("None").to_string(),
        ],
    )?;

    Ok(())
}

// Function to initialize the SQLite database with Git commit data
fn init_db(repo: &Repository, revwalk: Revwalk, tags: Vec<GitTag>) -> Result<Connection, SqlError> {
    // Open an in-memory SQLite database
    let conn = Connection::open_in_memory()?;

    // Create the 'commits' table
    conn.execute(
        "CREATE TABLE commits (
                        id       TEXT PRIMARY KEY,
                        author   TEXT NOT NULL,
                        date     TEXT NOT NULL,
                        message  TEXT NOT NULL
                    )",
        (),
    )?;

    // Create the 'tags' table
    conn.execute(
        "CREATE TABLE tags (
                        id          TEXT PRIMARY KEY,
                        name        TEXT NOT NULL,
                        target_id   TEXT NOT NULL,
                        target_type TEXT NOT NULL,
                        tagger      TEXT NOT NULL,
                        message     TEXT NOT NULL
                    )",
        (),
    )?;

    // Iterate over Git commit history and insert each commit into the database
    for commit_id in revwalk {
        let commit_id = commit_id.expect("Failed to get commit ID");
        let commit = repo.find_commit(commit_id).expect("Failed to find commit");

        insert_commit(&conn, &commit)?;
    }

    // Insert tags
    for tag in tags {
        insert_tag(&conn, &tag)?;
    }

    Ok(conn)
}

// Function to convert SQLite Value to a String
fn value_to_string(value: Value) -> String {
    match value {
        Value::Integer(i) => i.to_string(),
        Value::Real(f) => f.to_string(),
        Value::Text(s) => s,
        Value::Blob(_) => String::from("Blob"),
        Value::Null => String::from("NULL"),
    }
}

// Function to run an SQL query and display the results in a table
fn run_sql_query(conn: &Connection, sql: &str) -> Result<(), SqlError> {
    let mut stmt = conn.prepare(sql)?;
    let column_names: Vec<&str> = stmt.column_names().into_iter().collect();
    let column_len = column_names.len();

    // Create a comfy_table for displaying query results
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        // TODO: make table width configurable
        .set_width(80)
        .set_header(&column_names);

    // Execute the SQL query
    let mut rows = stmt.query([])?;
    let mut row_count = 0;

    // Iterate over the query results and add rows to the table
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

    // Print the table and the row count
    println!("{table}");
    println!("Rows returned: {}", row_count);

    Ok(())
}

// Function to get all tags
fn get_all_tags(repo: &Repository) -> Result<Vec<GitTag>, GitError> {
    let mut tags = Vec::new();

    repo.tag_foreach(|id, name| {
        let tag = repo.find_tag(id);

        match tag {
            // Annotated tag
            Ok(t) => tags.push(GitTag {
                id: t.id(),
                name: t.name(),
                target_id: t.target_id(),
                target_type: t.target_type(),
                tagger: t.tagger(),
                message: t.message(),
            }),
            // Lightweight tag
            _ => tags.push(GitTag {
                id,
                name: Some(std::str::from_utf8(&name.to_owned()).unwrap_or("None")),
                target_id: id,
                target_type: Some(ObjectType::Commit),
                tagger: None,
                message: None,
            }),
        };

        // Continue iterating over tags
        true
    })?;

    Ok(tags)
}

// Constants for the terminal prompt and the initial SQL query
const TERMINAL_PROMPT: &str = ">> ";
const INIT_SQL_QUERY: &str = "SELECT * FROM COMMITS ORDER BY date DESC LIMIT 1;";

fn main() -> Result<(), String> {
    // TODO: take repo_path as an option
    let repo_path = "./";

    // Open the Git repository
    let repo = Repository::open(repo_path).map_err(|err| format!("Cannot open repo. {}", err))?;

    // Create a revwalk to traverse the commit history
    let mut revwalk = repo.revwalk().expect("Failed to create revwalk");
    revwalk.push_head().expect("Failed to push HEAD OID");

    // Get all tags
    let tags = get_all_tags(&repo).map_err(|err| format!("Cannot get tags. {}", err))?;

    // Initialize the SQLite database with Git commit data
    let conn = init_db(&repo, revwalk, tags).map_err(|err| format!("DB error. {}", err))?;

    // Run the initial SQL query and display the result
    println!("{}{}", TERMINAL_PROMPT, INIT_SQL_QUERY);
    run_sql_query(&conn, INIT_SQL_QUERY)
        .map_err(|err| format!("Initial SQL query failed. {}", err))?;

    // Command loop for running SQL queries from the user
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
