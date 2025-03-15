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

use portfolio_explorer::config::Config;
use portfolio_explorer::{adapter_factory::create_adapter, adapters::base::MetricsAdapter};
use std::collections::HashMap;
use tokio::time::{Duration, sleep};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_file("config.toml")?;
    let mut adapters: HashMap<String, Box<dyn MetricsAdapter + Send + Sync>> = HashMap::new();

    for (name, metric) in &config.metrics {
        if metric.enabled {
            match create_adapter(metric) {
                Ok(adapter) => {
                    adapters.insert(name.clone(), adapter);
                }
                Err(e) => eprintln!("Error initializing adapter {}: {}", name, e),
            }
        }
    }

    loop {
        for (name, adapter) in adapters.iter_mut() {
            if let Err(e) = adapter.update_params(None).await {
                eprintln!("Error updating {}: {}", name, e);
            }
            for param in adapter.get_params() {
                println!("{}_{}: {:?}", name, param, adapter.get_value(param).await);
            }
        }
        sleep(Duration::from_secs(10)).await;
    }
}
