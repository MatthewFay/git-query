use chrono::{TimeZone, Utc};
use comfy_table::Table;
use comfy_table::{presets::UTF8_FULL, ContentArrangement};
use git2::{
    Branch, BranchType, Commit as GitCommit, ObjectType, Oid, Repository, Revwalk, Tag, Time,
};
use rusqlite::params;
use rusqlite::{types::Value, Connection, Result};
use std::fmt;
use std::io::{stdin, stdout, Write};

// Enum to support both annotated and lightweight git tags
enum GitTag<'a> {
    Annotated(Tag<'a>),
    Lightweight {
        id: Oid,
        name: Option<String>,
        target_id: Oid,
    },
}

// Enum for errors
#[derive(Debug)]
enum Error {
    GitError(git2::Error),
    SqlError(rusqlite::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GitError(err) => write!(f, "Git error: {}", err),
            Self::SqlError(err) => write!(f, "SQL error: {}", err),
        }
    }
}

impl From<git2::Error> for Error {
    fn from(err: git2::Error) -> Self {
        Error::GitError(err)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Error::SqlError(err)
    }
}

// Function to insert a Git commit into the SQLite database
fn insert_commit(conn: &Connection, commit: &GitCommit) -> Result<(), Error> {
    // Extract the commit datetime in UTC
    let datetime = Utc.timestamp_opt(commit.time().seconds(), 0);

    // Execute the SQL INSERT statement
    conn.execute(
        "INSERT OR IGNORE INTO commits (id, author, date, message) VALUES (?1, ?2, ?3, ?4)",
        params![
            // Store only the first 7 characters of the commit id
            commit.id().to_string().chars().take(7).collect::<String>(),
            commit.author().name(),
            datetime.unwrap().to_string(),
            commit.message(),
        ],
    )?;

    Ok(())
}

// Function to remove PGP signature from message
fn remove_pgp_signature(message: &str) -> String {
    let begin_pgp_marker = "-----BEGIN PGP SIGNATURE-----";

    // Find the position of the PGP marker
    let end_pos = message.find(begin_pgp_marker);

    if let Some(e_pos) = end_pos {
        // Take a substring ending with the position of the PGP marker
        let modified_message = message[..e_pos].trim().to_string();

        modified_message
    } else {
        message.to_string()
    }
}

// Function to insert a Git tag into the SQLite database
fn insert_tag(conn: &Connection, tag: GitTag) -> Result<(), Error> {
    match tag {
        GitTag::Annotated(t) => {
            let tagger: Option<String> = t
                .tagger()
                .and_then(|sig| sig.name().map(|name| name.to_string()));

            let date = t
                .tagger()
                .map(|sig| sig.when())
                .map(|time: Time| Utc.timestamp_opt(time.seconds(), 0).unwrap().to_string());

            conn.execute(
                "INSERT INTO tags (id, name, target_id, target_type, tagger, date, message) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    // Store only the first 7 characters of the tag id
                    t.id().to_string().chars().take(7).collect::<String>(),
                    t.name(),
                    // Store only the first 7 characters of the tag target id
                    t.target_id().to_string().chars().take(7).collect::<String>(),
                    t.target_type().map(|t_type| t_type.to_string()),
                    tagger,
                    date,
                    t.message().map(remove_pgp_signature),
                ],
            )?;
        }
        GitTag::Lightweight {
            id,
            name,
            target_id,
        } => {
            conn.execute(
                "INSERT INTO tags (id, name, target_id, target_type) VALUES (?1, ?2, ?3, ?4)",
                params![
                    // Store only the first 7 characters of the tag id
                    id.to_string().chars().take(7).collect::<String>(),
                    name,
                    // Store only the first 7 characters of the tag target id
                    target_id.to_string().chars().take(7).collect::<String>(),
                    ObjectType::Commit.to_string(),
                ],
            )?;
        }
    }

    Ok(())
}

// Function to insert a Git branch into the SQLite database
fn insert_branch(conn: &Connection, branch: Branch, branch_type: BranchType) -> Result<(), Error> {
    let reference = branch.get();
    let head_commit = reference.peel_to_commit().ok();
    let head_commit_id = head_commit
        .as_ref()
        .map(|h| h.id().to_string().chars().take(7).collect::<String>());
    let head_commit_date = head_commit.as_ref().map(|h| {
        Utc.timestamp_opt(h.time().seconds(), 0)
            .unwrap()
            .to_string()
    });

    conn.execute(
        "INSERT INTO branches (name, type, head_commit_id, head_commit_date) VALUES (?1, ?2, ?3, ?4)",
        params![
            branch.name().ok(),
            match branch_type {
                BranchType::Local => "local",
                BranchType::Remote => "remote",
            },
            head_commit_id,
            head_commit_date
        ],
    )?;

    Ok(())
}

// Function to initialize the SQLite database with Git commit data
fn init_db(repo: &Repository, revwalk: Revwalk) -> Result<Connection, Error> {
    // Open an in-memory SQLite database
    let conn = Connection::open_in_memory()?;

    // Create the 'commits' table
    conn.execute(
        "CREATE TABLE commits (
                        id       TEXT PRIMARY KEY,
                        author   TEXT,
                        date     TEXT NOT NULL,
                        message  TEXT
                    )",
        (),
    )?;

    // Create the 'tags' table
    conn.execute(
        "CREATE TABLE tags (
                        id          TEXT PRIMARY KEY,
                        name        TEXT,
                        target_id   TEXT NOT NULL,
                        target_type TEXT,
                        tagger      TEXT,
                        date        TEXT,
                        message     TEXT
                    )",
        (),
    )?;

    // Create the 'branches' table
    conn.execute(
        "CREATE TABLE branches (
                        name             TEXT,
                        type             TEXT,
                        head_commit_id   TEXT,
                        head_commit_date TEXT
                    )",
        (),
    )?;

    // Iterate over Git commit history and insert each commit into the database
    for commit_id in revwalk {
        let commit_id = commit_id.expect("Failed to get commit ID");
        let commit = repo.find_commit(commit_id).expect("Failed to find commit");

        insert_commit(&conn, &commit)?;
    }

    let mut tag_sql_error: Option<Error> = None;

    // Insert tags
    repo.tag_foreach(|id, name| {
        let tag = repo.find_tag(id);

        match tag {
            // Annotated tag
            Ok(t) => {
                if let Err(err) = insert_tag(&conn, GitTag::Annotated(t)) {
                    tag_sql_error = Some(err);
                    return false; // Stop iterating over tags
                }
            }
            // Lightweight tag
            _ => {
                let n: Option<String> = std::str::from_utf8(name)
                    .map(|s| s.to_string())
                    .ok()
                    // Remove "refs/tags/" prefix, if present
                    .map(|s| s.strip_prefix("refs/tags/").unwrap_or(&s).to_string());

                if let Err(err) = insert_tag(
                    &conn,
                    GitTag::Lightweight {
                        id,
                        name: n,
                        target_id: id,
                    },
                ) {
                    tag_sql_error = Some(err);
                    return false; // Stop iterating over tags
                }
            }
        };

        // Continue iterating over tags
        true
    })
    .expect("Tags should be iterable");

    if let Some(tag_sql_err) = tag_sql_error {
        return Err(tag_sql_err);
    }

    // Insert branches
    for branch in repo.branches(None).expect("Branches should be iterable") {
        let b = branch.expect("Branch should be valid");
        insert_branch(&conn, b.0, b.1)?;
    }

    Ok(conn)
}

// Function to convert SQLite Value to a String
fn value_to_string(value: Value) -> String {
    match value {
        Value::Integer(i) => i.to_string(),
        Value::Real(f) => f.to_string(),
        // Replace \r\n with \n, as \r\n causes formatting issues with table
        Value::Text(s) => s.replace("\r\n", "\n"),
        Value::Blob(_) => String::from("Blob"),
        Value::Null => String::from("NULL"),
    }
}

// Function to run an SQL query and display the results in a table
fn run_sql_query(conn: &Connection, sql: &str) -> Result<(), Error> {
    let mut stmt = conn.prepare(sql)?;
    let column_names: Vec<&str> = stmt.column_names().into_iter().collect();
    let column_len = column_names.len();

    // Create a comfy_table for displaying query results
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        // TODO: make table width configurable
        // .set_width(80)
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

    // Show tip if no results returned and SQL query contains `commits`
    if row_count == 0 && sql.contains("commits") {
        println!("Tip: use the `traverse <commit id>` command to insert commit history")
    }

    Ok(())
}

fn traverse(conn: &Connection, repo: &Repository, commit_id: &str) -> Result<(), Error> {
    // Create a revwalk to traverse the commit history
    let mut revwalk = repo.revwalk()?;
    let commit = repo.find_commit_by_prefix(commit_id)?;
    revwalk.push(commit.id())?;

    // Iterate over Git commit history and insert each commit into the database
    for commit_id in revwalk {
        let commit_id = commit_id?;
        let commit = repo.find_commit(commit_id)?;

        insert_commit(&conn, &commit)?;
    }

    Ok(())
}

// Constants for the terminal prompt and the initial SQL query
const TERMINAL_PROMPT: &str = ">> ";
const INIT_SQL_QUERY: &str = "SELECT * FROM commits ORDER BY date DESC LIMIT 1;";

fn main() -> Result<(), String> {
    // TODO: take repo_path as an option
    let repo_path = "./";

    // Open the Git repository
    let repo = Repository::open(repo_path).map_err(|err| format!("Cannot open repo. {}", err))?;

    // Create a revwalk to traverse the commit history
    let mut revwalk = repo.revwalk().expect("Failed to create revwalk");
    revwalk.push_head().expect("Failed to push HEAD OID");

    // Initialize the SQLite database with Git commit data
    let conn = init_db(&repo, revwalk).map_err(|err| format!("DB error. {}", err))?;

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

        match input.split_whitespace().collect::<Vec<&str>>().as_slice() {
            [""] => {}
            ["exit"] | ["quit"] => break,
            ["help"] => {
                println!("Available commands:");
                println!(" - `exit` or `quit`: Exit the program.");
                println!(" - `help`: Display this help message.");
                println!(" - `traverse <commit id>`: Traverse commit history and insert each commit into the database.");
                println!(" - Enter SQL at the prompt to see results.");
            }
            ["traverse", commit_id] => {
                if let Err(err) = traverse(&conn, &repo, commit_id) {
                    eprintln!("traverse error. {}", err);
                }
            }
            _ => {
                if let Err(err) = run_sql_query(&conn, input) {
                    eprintln!("SQL error. {}", err);
                }
            }
        }
    }

    Ok(())
}
