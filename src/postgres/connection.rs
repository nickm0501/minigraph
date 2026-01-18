use tokio_postgres::{Client, NoTls};

use crate::postgres::PostgresConfig;

pub async fn connect(config: &PostgresConfig) -> Result<Client, tokio_postgres::Error> {
    // Important: do not log `DATABASE_URL` directly (it may include the password).
    // We rely on `tokio-postgres` to parse it.
    let (client, connection) = tokio_postgres::connect(&config.database_url, NoTls).await?;

    tokio::spawn(async move {
        if let Err(err) = connection.await {
            eprintln!("[PG][ERR] connection error: {err}");
        }
    });

    Ok(client)
}
