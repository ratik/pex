use std::{collections::HashMap, sync::Arc};

use prometheus::{Gauge, IntGauge, Registry, core::GenericGauge};
use tokio::sync::Mutex;

#[async_trait::async_trait]
pub trait MetricsAdapter {
    fn get_name(&self) -> &str;

    async fn update_params(
        &mut self,
        metrics: Arc<Mutex<HashMap<String, Value>>>,
    ) -> Result<(), Box<dyn std::error::Error>>;

    fn get_key(&self, param_name: &str) -> String {
        format!("{}_{}", self.get_name(), param_name)
    }

    async fn register_param(
        &self,
        registry: Arc<Registry>,
        metrics: Arc<Mutex<HashMap<String, Value>>>,
        param_name: &str,
        param_type: ValueType,
    ) -> () {
        let key = self.get_key(param_name);
        let mut metrics = metrics.lock().await;
        match param_type {
            ValueType::Int => {
                let gauge = IntGauge::new(key.clone(), format!("Value of {}", key)).unwrap();
                registry.register(Box::new(gauge.clone())).unwrap();
                metrics.insert(key.clone(), Value::Int(gauge));
            }
            ValueType::Float => {
                let gauge = Gauge::new(key.clone(), format!("Value of {}", key)).unwrap();
                registry.register(Box::new(gauge.clone())).unwrap();
                metrics.insert(key.clone(), Value::Float(gauge));
            }
        }
    }
}

#[derive(Debug)]
pub enum Value {
    Int(GenericGauge<prometheus::core::AtomicI64>),
    Float(GenericGauge<prometheus::core::AtomicF64>),
}

pub enum ValueType {
    Int,
    Float,
}
