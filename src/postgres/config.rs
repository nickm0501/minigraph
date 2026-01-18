use std::env;

#[derive(Debug, Clone)]
pub struct PostgresConfig {
    pub database_url: String,
    pub publication_name: String,
    pub slot_name_base: String,
    pub slot_name_suffix: Option<String>,
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
}
