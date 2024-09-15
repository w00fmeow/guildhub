use super::middlewares::{optional_auth, require_auth};
use super::{controller, Event};
use crate::configuration::{Configuration, Environment};
use crate::libs::gitlab_api::gitlab_api::Member;
use crate::libs::gitlab_api::GitlabApi;
use crate::libs::health_checker::HealthChecker;
use crate::libs::mongo::database::MongoDatabase;
use crate::modules::auth::AuthService;
use crate::modules::gitlab::GitlabService;
use crate::modules::guild::{self, GuildsRepository, GuildsService};
use crate::modules::topic::{self, TopicsRepository, TopicsService};
use axum::middleware;
use axum::routing::{delete, get, post, put};
use axum::Router;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::{self, Receiver, Sender};
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

pub struct App {
    pub events_channel: (Sender<Event>, Receiver<Event>),
    pub configuration: Arc<Configuration>,
    pub database: Arc<MongoDatabase>,
    pub gitlab_service: Arc<GitlabService>,
    pub guilds_repository: Arc<GuildsRepository>,
    pub guilds_service: Arc<GuildsService>,
    pub topics_repository: Arc<TopicsRepository>,
    pub topics_service: Arc<TopicsService>,
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

        let events_channel = broadcast::channel::<Event>(10);

        let database =
            Arc::new(MongoDatabase::new(configuration.mongo.clone()));

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
            _ => {
                match gitlab_service.refresh_members_cache().await {
                    Err(err) => {
                        panic!("Failed to load members cache: {err}");
                    }
                    _ => {
                        info!("In memory cache for gitlab members is ready to go 🔥")
                    }
                }
            }
        }

        let topics_repository =
            Arc::new(TopicsRepository::new(database.clone()).await);
        let topics_service = Arc::new(TopicsService::new(
            gitlab_service.clone(),
            topics_repository.clone(),
        ));

        let guilds_repository =
            Arc::new(GuildsRepository::new(database.clone()).await);
        let guilds_service = Arc::new(GuildsService::new(
            topics_service.clone(),
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

        let app_events_sender = events_channel.0.clone();

        let guilds_service_ref = guilds_service.clone();

        tokio::spawn(async move {
            let mut guild_events_receiver =
                guilds_service_ref.events_channel.0.subscribe();

            loop {
                match guild_events_receiver.recv().await {
                    Ok(event) => {
                        let _ = app_events_sender.send(event.into());
                    }
                    Err(err) => error!("{err}"),
                }
            }
        });

        let app_events_sender = events_channel.0.clone();

        let topics_service_ref = topics_service.clone();

        tokio::spawn(async move {
            let mut topic_events_receiver =
                topics_service_ref.events_channel.0.subscribe();

            loop {
                match topic_events_receiver.recv().await {
                    Ok(event) => {
                        let _ = app_events_sender.send(event.into());
                    }
                    Err(err) => error!("{err}"),
                }
            }
        });

        App {
            events_channel,
            configuration,
            database,
            dependencies,
            gitlab_service,
            auth_service,
            guilds_repository,
            guilds_service,
            topics_repository,
            topics_service,
        }
    }

    pub fn get_app_router(app: Arc<App>) -> Router {
        let assets_dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static");

        let guild_router = Router::new()
            .route("/", get(guild::get_guilds_page))
            .route("/", post(guild::create_guild))
            .route("/list", get(guild::get_guilds_list))
            .route("/create", get(guild::get_create_guild_form))
            .route("/draft", post(guild::post_guild_form_draft))
            .route("/draft/members/:member_id", delete(guild::remove_member))
            .route("/draft/members/:member_id", post(guild::insert_new_member))
            .route("/:guild_id", get(guild::get_guild))
            .route("/:guild_id", delete(guild::delete_guild))
            .route("/:guild_id", put(guild::update_guild))
            .route("/:guild_id/events", get(guild::subscribe_to_events))
            .route("/:guild_id/draft", post(guild::post_guild_form_draft))
            .route(
                "/:guild_id/draft/members/:member_id",
                post(guild::insert_new_member),
            )
            .route("/:guild_id/edit", get(guild::get_edit_guild_form))
            .route("/:guild_id/overview", get(guild::get_guild_overview))
            .route("/:guild_id/topics", get(topic::get_topics_list))
            .route("/:guild_id/topics", post(topic::create_topic))
            .route("/:guild_id/topics/add", get(topic::get_create_topic_form))
            .route(
                "/:guild_id/topics/draft",
                post(topic::post_topic_form_draft),
            )
            .route(
                "/:guild_id/topics/:topic_id/card",
                get(topic::get_topic_card),
            )
            .route(
                "/:guild_id/topics/:topic_id/draft",
                post(topic::post_topic_form_draft),
            )
            .route(
                "/:guild_id/topics/:topic_id",
                get(topic::get_edit_topic_form),
            )
            .route("/:guild_id/topics/:topic_id", put(topic::update_topic))
            .route(
                "/:guild_id/topics/:topic_id/vote",
                post(topic::upvote_topic),
            )
            .route("/:guild_id/topics/:topic_id", delete(topic::delete_topic))
            .route(
                "/:guild_id/topics/:topic_id/vote",
                delete(topic::remove_vote_from_topic),
            )
            .route_layer(middleware::from_fn_with_state(
                app.clone(),
                require_auth,
            ));

        let public_router = Router::new()
            .route("/", get(controller::index))
            .route("/health", get(controller::health))
            .route("/login", get(controller::login))
            .route("/login", delete(controller::logout))
            .route("/gitlab_auth", get(controller::gitlab_auth));

        Router::new()
            .merge(public_router)
            .nest("/guilds", guild_router)
            .nest_service("/static", ServeDir::new(assets_dir))
            .route_layer(middleware::from_fn_with_state(
                app.clone(),
                optional_auth,
            ))
            .with_state(app)
            .layer(TraceLayer::new_for_http())
    }

    pub fn is_healthy(&self) -> bool {
        return self.dependencies.iter().all(|service| service.get_health());
    }
}
