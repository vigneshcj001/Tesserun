use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
pub struct Connection {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_endpoint")]
    pub endpoint: String,
}
pub fn default_endpoint() -> String {
    "http://127.0.0.1:11434".into()
}
impl Default for Connection {
    fn default() -> Self {
        Self {
            provider: "ollama".into(),
            model: String::new(),
            api_key: String::new(),
            endpoint: default_endpoint(),
        }
    }
}
