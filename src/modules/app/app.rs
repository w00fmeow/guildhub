use super::auth_middleware::RequireAuth;
use super::controller;
use super::errors::format_error_response;
use crate::configuration::{Configuration, Environment};
use crate::libs::gitlab_api::gitlab_api::Member;
use crate::libs::gitlab_api::GitlabApi;
use crate::libs::health_checker::HealthChecker;
use crate::libs::mongo::database::MongoDatabase;
use crate::modules::auth::AuthService;
use crate::modules::gitlab::GitlabService;
use crate::modules::guild::{self, GuildsRepository, GuildsService};
use actix_files as fs;
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceFactory, ServiceRequest, ServiceResponse};
use actix_web::middleware::ErrorHandlers;
use actix_web::{error, middleware::Logger, web, App as ActixApp, HttpResponse};
use oauth2::http::StatusCode;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;
use tracing_subscriber::EnvFilter;

pub struct App {
    pub configuration: Arc<Configuration>,
    pub database: Arc<MongoDatabase>,
    pub gitlab_service: Arc<GitlabService>,
    pub guilds_repository: Arc<GuildsRepository>,
    pub guilds_service: Arc<GuildsService>,
    pub auth_service: Arc<AuthService>,
    pub dependencies: Arc<Vec<Box<Arc<dyn HealthChecker + Send + Sync>>>>,
}

impl App {
    pub async fn new() -> Self {
        let configuration = Arc::new(Configuration::new().await);

        App::init(configuration.clone()).await
    }

    pub async fn init(configuration: Arc<Configuration>) -> Self {
        let log_builder = tracing_subscriber::fmt::Subscriber::builder()
            .with_env_filter(EnvFilter::from_default_env());

        match configuration.environment {
            Environment::Production => {
                let _ = log_builder.json().try_init();
            }
            _ => {
                let _ = log_builder.pretty().try_init();
            }
        };

        let database = Arc::new(MongoDatabase::new(configuration.mongo.clone()));

        let database_ping_ref = Arc::clone(&database);

        tokio::spawn(async move {
            database_ping_ref
                .refresh_is_healthy_in_loop(Duration::from_secs(2))
                .await
        });

        let auth_service = Arc::new(AuthService::new(
            configuration.auth.expire_in_hours,
            configuration.auth.secret.clone(),
        ));

        let gitlab = GitlabApi::new(
            configuration.gitlab.access_token.clone(),
            configuration.gitlab.domain.clone(),
            configuration.gitlab.client_id.clone(),
            configuration.gitlab.client_secret.clone(),
            configuration.gitlab.redirect_url.clone(),
        );

        let gitlab_service = Arc::new(GitlabService::new(
            gitlab,
            configuration.gitlab.group_id.clone(),
            Duration::from_secs(60 * 10),
        ));

        match configuration.environment {
            Environment::Development => {
                gitlab_service
                    .insert_member_into_cache(Member::default())
                    .await
            }
            _ => match gitlab_service.refresh_members_cache().await {
                Err(err) => {
                    panic!("Failed to load members cache: {err}");
                }
                _ => {
                    info!("In memory cache for gitlab members is ready to go 🔥")
                }
            },
        }

        let guilds_repository = Arc::new(GuildsRepository::new(database.clone()).await);
        let guilds_service = Arc::new(GuildsService::new(
            guilds_repository.clone(),
            gitlab_service.clone(),
        ));

        let gitlab_service_ref = gitlab_service.clone();
        tokio::spawn(async move {
            gitlab_service_ref
                .refresh_cache_loop(Duration::from_secs(60 * 5))
                .await
        });

        let dependencies: Arc<Vec<Box<Arc<dyn HealthChecker + Send + Sync>>>> =
            Arc::new(vec![Box::new(database.clone())]);

        App {
            configuration,
            database,
            dependencies,
            gitlab_service,
            auth_service,
            guilds_repository,
            guilds_service,
        }
    }

    pub fn get_actix_app(
        app: Arc<App>,
    ) -> ActixApp<
        impl ServiceFactory<
            ServiceRequest,
            Config = (),
            Response = ServiceResponse<impl MessageBody>,
            Error = error::Error,
            InitError = (),
        >,
    > {
        let assets_path = std::env::current_dir().unwrap();
        let assets_path = assets_path.to_str().unwrap();

        ActixApp::new()
            .app_data(web::Data::new(app.clone()))
            .app_data(web::JsonConfig::default().error_handler(|err, _req| {
                error::InternalError::from_response(
                    "",
                    HttpResponse::BadRequest()
                        .content_type("application/json")
                        .body(format_error_response(&err.to_string())),
                )
                .into()
            }))
            .wrap(
                ErrorHandlers::new()
                    .handler(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        controller::add_internal_server_error_to_response,
                    )
                    .handler(
                        StatusCode::UNAUTHORIZED,
                        controller::add_internal_server_error_to_response,
                    ),
            )
            .route("/", web::get().to(controller::index))
            .route("/health", web::get().to(controller::health))
            .route("/login", web::get().to(controller::login))
            .route("/login", web::delete().to(controller::logout))
            .route("/gitlab_auth", web::get().to(controller::gitlab_auth))
            .service(
                web::scope("/guilds")
                    .wrap(RequireAuth)
                    .route("", web::get().to(guild::get_guilds_list))
                    .route("", web::post().to(guild::create_guild))
                    .route("/create", web::get().to(guild::get_create_guild_form))
                    .route("/draft", web::post().to(guild::post_guild_form_draft))
                    .route(
                        "/draft/members/{member_id}",
                        web::delete().to(guild::remove_member),
                    )
                    .route(
                        "/draft/members/{member_id}",
                        web::post().to(guild::insert_new_member),
                    )
                    .route("/{guild_id}", web::get().to(guild::get_guild))
                    .route("/{guild_id}", web::delete().to(guild::delete_guild))
                    .route("/{guild_id}", web::put().to(guild::update_guild))
                    .route(
                        "/{guild_id}/draft",
                        web::post().to(guild::post_guild_form_draft),
                    )
                    .route(
                        "/{guild_id}/draft/members/{member_id}",
                        web::post().to(guild::insert_new_member),
                    )
                    .route(
                        "/{guild_id}/edit",
                        web::get().to(guild::get_edit_guild_form),
                    )
                    .route(
                        "/{guild_id}/overview",
                        web::get().to(guild::get_guild_overview),
                    ),
            )
            .service(fs::Files::new(
                "/static",
                format!("{assets_path}/static").as_str(),
            ))
            .default_service(web::to(controller::not_found))
            .wrap(Logger::default())
    }

    pub fn is_healthy(&self) -> bool {
        return self.dependencies.iter().all(|service| service.get_health());
    }
}
