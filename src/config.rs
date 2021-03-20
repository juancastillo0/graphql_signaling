use serde;

#[derive(serde::Deserialize)]
pub struct Config {
    pub application_port: u16,
    pub database: DatabaseConfig,
}

#[derive(serde::Deserialize)]
pub struct DatabaseConfig {
    pub username: String,
    pub password: String,
    pub host: String,
    pub port: u16,
    pub database_name: String,
}

impl DatabaseConfig {
    pub fn url(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.username, self.password, self.host, self.port, self.database_name
        )
    }
}

pub fn get_config() -> Config {
    Config {
        application_port: 8080,
        database: DatabaseConfig {
            database_name: "webrtc_graphql".to_string(),
            host: "localhost".to_string(),
            password: "postgres".to_string(),
            username: "postgres".to_string(),
            port: 5432,
        },
    }
}
