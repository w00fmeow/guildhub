use chrono::Duration;
use mongodb::options::ClientOptions;
use serde::Deserialize;
use std::env;
use std::sync::Arc;
use std::time::Duration as StdDuration;

#[derive(Deserialize, Debug)]
pub enum Environment {
    Production,
    Test,
    Development,
}

impl Environment {
    pub fn is_test(&self) -> bool {
        matches!(self, Environment::Test)
    }
}

#[derive(Deserialize, Debug)]
pub struct MongoConfiguration {
    pub client_options: ClientOptions,
}

impl MongoConfiguration {
    pub async fn parse_connection_string(
        connection_string: &str,
    ) -> Result<ClientOptions, mongodb::error::Error> {
        let mut client_options = ClientOptions::parse(connection_string).await?;

        client_options.connect_timeout = Some(StdDuration::from_secs(5));

        if client_options.default_database.is_none() {
            client_options.default_database = Some(String::from("guildhub"));
        }

        Ok(client_options)
    }
}

#[derive(Debug)]
pub struct GitlabConfiguration {
    pub domain: String,
    pub access_token: String,
    pub group_id: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
}

impl GitlabConfiguration {
    pub fn new() -> Self {
        Self {
            domain: env::var("GITLAB_DOMAIN").expect("GITLAB_DOMAIN variable to be available"),

            access_token: env::var("GITLAB_ACCESS_TOKEN")
                .expect("GITLAB_ACCESS_TOKEN variable to be available"),

            group_id: env::var("GITLAB_GROUP_ID")
                .expect("GITLAB_GROUP_ID variable to be available"),

            client_id: env::var("GITLAB_CLIENT_ID")
                .expect("GITLAB_CLIENT_ID variable to be available"),

            client_secret: env::var("GITLAB_CLIENT_SECRET")
                .expect("GITLAB_CLIENT_SECRET variable to be available"),

            redirect_url: env::var("GITLAB_REDIRECT_URL")
                .expect("GITLAB_REDIRECT_URL variable to be available"),
        }
    }
}

#[derive(Debug)]
pub struct AuthConfiguration {
    pub expire_in_hours: Duration,
    pub secret: String,
}

impl AuthConfiguration {
    pub fn new() -> Self {
        Self {
            expire_in_hours: Duration::hours(
                env::var("AUTH_TOKEN_VALID_FOR_HOURS")
                    .expect("AUTH_TOKEN_VALID_FOR_HOURS variable to be available")
                    .parse()
                    .expect("AUTH_TOKEN_VALID_FOR_HOURS variable to an integer"),
            ),

            secret: env::var("AUTH_SECRET").expect("AUTH_SECRET variable to be available"),
        }
    }
}

#[derive(Debug)]
pub struct Configuration {
    pub mongo: Arc<MongoConfiguration>,
    pub app_port: u16,
    pub environment: Environment,
    pub gitlab: GitlabConfiguration,
    pub auth: AuthConfiguration,
}

impl Configuration {
    pub async fn new() -> Self {
        dotenv::dotenv().ok();

        let environment = parse_env();

        let mongo_db_uri =
            env::var("MONGO_DB_URI").unwrap_or_else(|_| String::from("mongodb://localhost:27017"));

        Configuration {
            mongo: Arc::new(MongoConfiguration {
                client_options: MongoConfiguration::parse_connection_string(&mongo_db_uri)
                    .await
                    .expect("Valid mongo_db_uri"),
            }),
            app_port: env::var("APP_PORT")
                .unwrap()
                .parse()
                .expect("APP_PORT variable to an integer"),
            environment,
            gitlab: GitlabConfiguration::new(),
            auth: AuthConfiguration::new(),
        }
    }
}

fn parse_env() -> Environment {
    let env_var = env::var("ENV").unwrap_or_else(|_| "development".to_string());

    match env_var.trim().to_lowercase().as_str() {
        "production" => Environment::Production,
        "prod" => Environment::Production,
        "test" => Environment::Test,
        "development" => Environment::Development,
        _ => Environment::Development,
    }
}
