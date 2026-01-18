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
                write!(f, "Postgres setup query failed ({query}): {error}")
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
    let create_publication = format!(
        "CREATE PUBLICATION IF NOT EXISTS {} FOR TABLE documents, comments",
        config.publication_name
    );
    exec(client, &create_publication).await?;

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
