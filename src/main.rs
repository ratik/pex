use portfolio_explorer::adapters::base::Value;
use portfolio_explorer::config::Config;
use portfolio_explorer::{adapter_factory::create_adapter, adapters::base::MetricsAdapter};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};
use warp::Filter;

use futures::stream::{FuturesUnordered, StreamExt};
use tokio::sync::Semaphore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_file("config.toml")?;

    let registry = Arc::new(prometheus::Registry::new());
    let metrics = Arc::new(Mutex::new(HashMap::<String, Value>::new()));
    let adapters = Arc::new(Mutex::new(HashMap::<
        String,
        Box<dyn MetricsAdapter + Send + Sync>,
    >::new()));

    {
        let mut adapters_guard = adapters.lock().await;
        for (name, config) in &config.metrics {
            if config.enabled {
                match create_adapter(name.to_string(), metrics.clone(), config).await {
                    Ok(adapter) => {
                        adapters_guard.insert(name.clone(), adapter);
                    }
                    Err(e) => eprintln!("Error initializing adapter {}: {}", name, e),
                }
            }
        }
    }

    {
        let metrics_guard = metrics.lock().await;
        for metric in metrics_guard.values() {
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

    let semaphore = Arc::new(Semaphore::new(config.concurrency as usize));

    loop {
        let mut futures = FuturesUnordered::new();

        let adapters_clone = Arc::clone(&adapters);
        let metrics_clone = Arc::clone(&metrics);
        let semaphore_clone = Arc::clone(&semaphore);

        {
            let adapters_guard = adapters_clone.lock().await; // Lock only during iteration
            for (name, _) in adapters_guard.iter() {
                let name = name.clone();
                let adapters_clone = Arc::clone(&adapters);
                let metrics_clone = Arc::clone(&metrics_clone);
                let semaphore_clone = Arc::clone(&semaphore_clone);

                futures.push(tokio::spawn(async move {
                    let _permit = semaphore_clone.acquire().await.unwrap(); // Acquire a permit

                    let mut adapters_guard = adapters_clone.lock().await;
                    if let Some(adapter) = adapters_guard.get_mut(&name) {
                        match adapter.update_params(metrics_clone).await {
                            Ok(_) => println!("Updated {}", name),
                            Err(e) => eprintln!("Error updating {}: {}", name, e),
                        }
                    }
                }));
            }
        }

        while let Some(_) = futures.next().await {} // Wait for all tasks to finish
        sleep(Duration::from_secs(config.interval)).await;
    }
}
