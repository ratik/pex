use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex;

#[async_trait::async_trait]
pub trait MetricsAdapter {
    fn get_params(&self) -> &Vec<String>;
    async fn update_params(
        &mut self,
        _params: Option<Vec<String>>,
    ) -> Result<(), Box<dyn std::error::Error>>;
    fn get_values(&self) -> Arc<Mutex<HashMap<String, super::base::Value>>>;
    async fn get_value(&self, param: &str) -> Option<super::base::Value> {
        let values = self.get_values();
        let values = values.lock().await;
        values.get(param).cloned()
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    Int(i128),
    Float(f32),
}
