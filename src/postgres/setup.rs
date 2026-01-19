use std::fmt;

use tokio_postgres::Client;

use crate::postgres;

#[derive(Debug)]
pub enum SetupError {
    ConnectionFailed(tokio_postgres::Error),
    QueryFailed {
        query: String,
        error: tokio_postgres::Error,
    },
}

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetupError::ConnectionFailed(err) => write!(f, "Postgres connection failed: {err}"),
            SetupError::QueryFailed { query, error } => {
                if let Some(db_error) = error.as_db_error() {
                    let detail = db_error
                        .detail()
                        .map(|d| format!(" detail={d}"))
                        .unwrap_or_default();
                    let hint = db_error
                        .hint()
                        .map(|h| format!(" hint={h}"))
                        .unwrap_or_default();

                    write!(
                        f,
                        "Postgres setup query failed ({query}): code={} message={}{}{}",
                        db_error.code().code(),
                        db_error.message(),
                        detail,
                        hint
                    )
                } else {
                    write!(f, "Postgres setup query failed ({query}): {error}")
                }
            }
        }
    }
}

impl std::error::Error for SetupError {}

pub async fn setup_postgres(config: &postgres::PostgresConfig) -> Result<(), SetupError> {
    let client = postgres::connect(config)
        .await
        .map_err(SetupError::ConnectionFailed)?;

    run_setup_queries(&client, config).await
}

async fn run_setup_queries(
    client: &Client,
    config: &postgres::PostgresConfig,
) -> Result<(), SetupError> {
    // Keep this idempotent for local/dev ergonomics.
    // For Phase 1 we prefer simplicity over a full migration framework.

    exec(
        client,
        "CREATE TABLE IF NOT EXISTS documents (id TEXT PRIMARY KEY)",
    )
    .await?;

    // UUID defaults require an extension. `pgcrypto` is commonly available in local/dev
    // Postgres installs and provides `gen_random_uuid()`.
    exec(client, "CREATE EXTENSION IF NOT EXISTS pgcrypto").await?;

    exec(
        client,
        "CREATE TABLE IF NOT EXISTS comments (id UUID PRIMARY KEY DEFAULT gen_random_uuid(), document_id TEXT NOT NULL, text TEXT)",
    )
    .await?;

    // Always run the ALTER for simplicity; it is safe to repeat.
    exec(client, "ALTER TABLE comments REPLICA IDENTITY FULL").await?;

    // TODO: `publication_name` is an operator-provided env var used as an SQL identifier.
    // Identifiers can't be parameterized, so we should validate it (e.g., [a-zA-Z0-9_])
    // to avoid accidental bad names and prevent SQL injection via env vars.
    //
    // Also note: Postgres does not support `CREATE PUBLICATION IF NOT EXISTS`, so we do
    // a best-effort create and treat "already exists" as success.
    let create_publication = format!(
        "CREATE PUBLICATION {} FOR TABLE documents, comments",
        config.publication_name
    );

    match exec(client, &create_publication).await {
        Ok(()) => {}
        Err(SetupError::QueryFailed { query, error }) => {
            // Postgres 18 doesn't support IF NOT EXISTS for Publication
            // so treat a duplicate for a publication as a success
            if is_duplicate_object(&error) {
                crate::logging::vprintln(format_args!(
                    "[PG][SETUP] publication already exists: {}",
                    config.publication_name
                ));
            } else {
                return Err(SetupError::QueryFailed { query, error });
            }
        }
        Err(err) => return Err(err),
    }

    ensure_replication_slot(client, config).await?;

    Ok(())
}

fn is_duplicate_object(error: &tokio_postgres::Error) -> bool {
    // "duplicate_object" (42710) covers cases like "publication already exists".
    let Some(db_error) = error.as_db_error() else {
        return false;
    };

    db_error.code().code() == "42710"
}

async fn ensure_replication_slot(
    client: &Client,
    config: &postgres::PostgresConfig,
) -> Result<(), SetupError> {
    let slot_name = config.slot_name();

    // Slot names are SQL values here (not identifiers), so we can safely bind them.
    let slot_exists_query = "SELECT 1 FROM pg_replication_slots WHERE slot_name = $1";

    let exists = client
        .query_opt(slot_exists_query, &[&slot_name])
        .await
        .map_err(|error| SetupError::QueryFailed {
            query: slot_exists_query.to_string(),
            error,
        })?
        .is_some();

    if exists {
        crate::logging::vprintln(format_args!("[PG][SETUP] slot exists: {slot_name}"));
        return Ok(());
    }

    crate::logging::vprintln(format_args!("[PG][SETUP] creating slot: {slot_name}"));

    let create_slot_query =
        "SELECT slot_name, lsn::text FROM pg_create_logical_replication_slot($1, 'pgoutput')";

    // Create slot and log the consistent point (LSN) where it becomes usable.
    let row = client
        .query_one(create_slot_query, &[&slot_name])
        .await
        .map_err(|error| SetupError::QueryFailed {
            query: create_slot_query.to_string(),
            error,
        })?;

    let created_slot_name: String = row.get(0);
    let consistent_point: String = row.get(1);

    crate::logging::vprintln(format_args!(
        "[PG][SETUP] created slot: {created_slot_name} consistent_point={consistent_point}"
    ));

    Ok(())
}

async fn exec(client: &Client, query: &str) -> Result<(), SetupError> {
    crate::logging::vprintln(format_args!("[PG][SETUP] {query}"));

    client
        .execute(query, &[])
        .await
        .map(|_| ())
        .map_err(|error| SetupError::QueryFailed {
            query: query.to_string(),
            error,
        })
}
