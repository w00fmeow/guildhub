use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Location {
    pub path: String,
    pub select: String,
    pub target: String,
    pub swap: String,
}
