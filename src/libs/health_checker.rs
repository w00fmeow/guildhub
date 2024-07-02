use async_trait::async_trait;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

#[async_trait]
pub trait HealthChecker {
    fn get_service_name(&self) -> String;

    fn get_health(&self) -> bool;

    fn persist_new_health_status(&self, is_healthy: bool);

    async fn check_is_healthy(&self) -> Result<()>;

    async fn refresh_is_healthy_in_loop(&self, interval: Duration) {
        loop {
            let new_is_healthy;

            match self.check_is_healthy().await {
                Ok(()) => {
                    new_is_healthy = true;
                }
                Err(err) => {
                    error!("{err}");
                    new_is_healthy = false;
                }
            };

            let current_health_status = self.get_health();

            match (current_health_status, new_is_healthy) {
                (true, false) => warn!("{} is unhealthy", self.get_service_name()),
                (false, true) => info!("{} is healthy", self.get_service_name()),
                _ => {}
            }

            self.persist_new_health_status(new_is_healthy);

            sleep(interval).await;
        }
    }

    fn to_dependency(&self) -> Dependency {
        Dependency {
            name: self.get_service_name(),
            up: self.get_health(),
        }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct Dependency {
    pub name: String,
    pub up: bool,
}
