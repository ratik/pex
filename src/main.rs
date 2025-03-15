// use portfolio_explorer::adapters::{self, base::MetricsAdapter};

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     // let mut comp = adapters::compound::CompoundAdapter::new(
//     //     vec!["0xe26E8e942193f02dCfcaA798057Df696A3b79811"],
//     //     "0x48759F220ED983dB51fA7A8C0D2AAb8f3ce4166a",
//     //     "354563ff369a4d2ab6b4634b3e6809da",
//     // )?;
//     // let mut comp = adapters::erc20::Erc20Adapter::new(
//     //     vec!["0x6654C4cA46dB6003bf819803dD88eD6af118dcF9"],
//     //     "0xdAC17F958D2ee523a2206206994597C13D831ec7",
//     //     "354563ff369a4d2ab6b4634b3e6809da",
//     // )?;
//     let mut comp = adapters::cosmos_bank::CosmosBankAdapter::new(
//         vec!["celestia1x38l2kya94s69g3nc4hjp4yugfr2srm30whg0e"],
//         "https://celestia-rpc.polkachu.com:443/",
//         vec!["utia"],
//     )?;
//     comp.update_params(None).await?;
//     println!("{:?}", comp.get_values());
//     Ok(())
// }

use portfolio_explorer::adapters::base::Value;
use portfolio_explorer::config::Config;
use portfolio_explorer::{adapter_factory::create_adapter, adapters::base::MetricsAdapter};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

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

    loop {
        for (name, adapter) in adapters.iter_mut() {
            if let Err(e) = adapter.update_params(metrics.clone()).await {
                eprintln!("Error updating {}: {}", name, e);
            } else {
                println!("Updated {}", name);
            }
        }
        sleep(Duration::from_secs(10)).await;
    }
}
