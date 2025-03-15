use portfolio_explorer::adapters::base::Value;
use portfolio_explorer::config::Config;
use portfolio_explorer::{adapter_factory::create_adapter, adapters::base::MetricsAdapter};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};
use warp::Filter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_file("config.toml")?;
    let mut adapters: HashMap<String, Box<dyn MetricsAdapter + Send + Sync>> = HashMap::new();

    let registry = Arc::new(prometheus::Registry::new());
    let metrics = Arc::new(Mutex::new(HashMap::<String, Value>::new()));

    for (name, config) in &config.metrics {
        if config.enabled {
            match create_adapter(name.to_string(), metrics.clone(), config).await {
                Ok(adapter) => {
                    adapters.insert(name.clone(), adapter);
                }
                Err(e) => eprintln!("Error initializing adapter {}: {}", name, e),
            }
        }
    }
    {
        let metrics = metrics.clone();
        let metrics = metrics.lock().await;
        for metric in metrics.values() {
            match metric {
                Value::Int(gauge) => {
                    registry.register(Box::new(gauge.clone()))?;
                }
                Value::Float(gauge) => {
                    registry.register(Box::new(gauge.clone()))?;
                }
            }
        }
    }

    let registry_clone = registry.clone();
    tokio::spawn(async move {
        let metrics_route = warp::path!("metrics").map(move || {
            let mut buffer = String::new();
            let encoder = prometheus::TextEncoder::new();
            let metric_families = registry_clone.gather();
            encoder.encode_utf8(&metric_families, &mut buffer).unwrap();
            warp::reply::with_header(buffer, "Content-Type", "text/plain; version=0.0.4")
        });

        warp::serve(metrics_route).run(([0, 0, 0, 0], 9100)).await;
    });

    loop {
        for (name, adapter) in adapters.iter_mut() {
            if let Err(e) = adapter.update_params(metrics.clone()).await {
                eprintln!("Error updating {}: {}", name, e);
            } else {
                println!("Updated {}", name);
            }
        }
        sleep(Duration::from_secs(config.interval)).await;
    }
}
