use std::collections::HashMap;

#[async_trait::async_trait]
pub trait MetricsAdapter {
    fn get_params(&self) -> &Vec<String>;
    async fn update_params(
        &mut self,
        _params: Option<Vec<String>>,
    ) -> Result<(), Box<dyn std::error::Error>>;
    fn get_values(&self) -> &HashMap<String, super::base::Value>;
    fn get_value(&self, param: &str) -> Option<super::base::Value> {
        self.get_values().get(param).cloned()
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    Int(i128),
    Float(f32),
}
