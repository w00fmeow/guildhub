use serde::Serialize;

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
