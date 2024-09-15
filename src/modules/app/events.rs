use serde::{Deserialize, Serialize};

use crate::modules::{guild::GuildEvent, topic::types::TopicEvent};

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToastLevel {
    Info,
    Warning,
    Error,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HxTriggerEvent {
    ShowToast { level: ToastLevel, message: String },
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum Event {
    Topic(TopicEvent),
    Guild(GuildEvent),
}
