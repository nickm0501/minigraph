use std::env;

#[derive(Debug, Clone)]
pub struct PostgresConfig {
    pub database_url: String,
    pub publication_name: String,
    pub slot_name_base: String,
    pub slot_name_suffix: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: Option<String>,
    pub database: String,
}

impl PostgresConfig {
    pub fn from_env() -> Self {
        // Keep local/dev easy: DATABASE_URL is optional and defaults to localhost.
        // In production, prefer setting DATABASE_URL explicitly.
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://postgres:postgres@localhost:5432/mini_graph".to_string()
        });

        let publication_name =
            env::var("MINI_GRAPH_PUBLICATION").unwrap_or_else(|_| "mini_graph_pub".to_string());

        let slot_name_base =
            env::var("MINI_GRAPH_SLOT").unwrap_or_else(|_| "mini_graph_slot".to_string());

        let slot_name_suffix = env::var("MINI_GRAPH_SLOT_SUFFIX").ok();

        Self {
            database_url,
            publication_name,
            slot_name_base,
            slot_name_suffix,
        }
    }

    pub fn slot_name(&self) -> String {
        let Some(suffix) = self.slot_name_suffix.as_ref() else {
            return self.slot_name_base.clone();
        };

        let suffix = suffix.trim();
        if suffix.is_empty() {
            return self.slot_name_base.clone();
        }

        let suffix = suffix.trim_start_matches('_');
        format!("{}_{}", self.slot_name_base, suffix)
    }

    pub fn connection_info(&self) -> Result<ConnectionInfo, url::ParseError> {
        let url = url::Url::parse(&self.database_url)?;

        let host = url.host_str().unwrap_or("localhost").to_string();
        let port = url.port().unwrap_or(5432);

        let user = if url.username().is_empty() {
            "postgres".to_string()
        } else {
            url.username().to_string()
        };

        let password = url.password().map(|p| p.to_string());

        // url.path() includes a leading '/', so strip it.
        let database = url.path().strip_prefix('/').unwrap_or(url.path());
        let database = if database.is_empty() {
            "postgres".to_string()
        } else {
            database.to_string()
        };

        Ok(ConnectionInfo {
            host,
            port,
            user,
            password,
            database,
        })
    }
}
