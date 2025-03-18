use crate::adapters::btc::BTCAdapter;
use crate::adapters::cosmwasm_smart_query::CosmWasmSmartQueryAdapter;
use crate::adapters::eth::ETHAdapter;
use crate::adapters::{
    base::MetricsAdapter, compound::CompoundAdapter, cosmos_bank::CosmosBankAdapter,
    erc20::Erc20Adapter,
};
use crate::config::MetricConfig;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn create_adapter(
    name: String,
    metrics: Arc<Mutex<HashMap<String, super::adapters::base::Value>>>,
    config: &MetricConfig,
) -> Result<Box<dyn MetricsAdapter + Send + Sync>, Box<dyn Error>> {
    println!("Creating adapter: {}", config.adapter);
    match config.adapter.as_str() {
        "btc" => {
            let addresses = config.config["addresses"]
                .as_array()
                .ok_or("Missing addresses")?
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<Vec<_>>();

            Ok(Box::new(BTCAdapter::new(&name, metrics, addresses).await?))
        }
        "compound" | "erc20" | "eth" => {
            let addresses = config.config["addresses"]
                .as_array()
                .ok_or("Missing addresses")?
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<Vec<_>>();

            let infura_token = config.config["infura_token"]
                .as_str()
                .ok_or("Missing infura_token")?;

            let decimals: u8 = config.config["decimals"].as_u64().unwrap() as u8 - 2;

            match config.adapter.as_str() {
                "compound" => {
                    let contract = config.config["contract"]
                        .as_str()
                        .ok_or("Missing contract")?;
                    Ok(Box::new(
                        CompoundAdapter::new(
                            &name,
                            metrics,
                            addresses,
                            contract,
                            infura_token,
                            decimals,
                        )
                        .await?,
                    ))
                }
                "erc20" => {
                    let contract = config.config["contract"]
                        .as_str()
                        .ok_or("Missing contract")?;
                    Ok(Box::new(
                        Erc20Adapter::new(
                            &name,
                            metrics,
                            addresses,
                            contract,
                            infura_token,
                            decimals,
                        )
                        .await?,
                    ))
                }
                "eth" => Ok(Box::new(
                    ETHAdapter::new(&name, metrics, addresses, infura_token, decimals).await?,
                )),
                _ => unreachable!(),
            }
        }
        "cosmos_bank" => {
            let addresses = config.config["addresses"]
                .as_array()
                .ok_or("Missing addresses")?
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<Vec<_>>();

            let denoms = config.config["denoms"]
                .as_array()
                .ok_or("Missing denoms")?
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<Vec<_>>();

            let rpc_endpoint = config.config["rpc"].as_str().ok_or("Missing rpc")?;

            Ok(Box::new(
                CosmosBankAdapter::new(&name, metrics, addresses, rpc_endpoint, denoms).await?,
            ))
        }
        "cosmwasm_smart_query" => {
            let contract = config.config["contract"]
                .as_str()
                .ok_or("Missing contract")?;

            let rpc_endpoint = config.config["rpc"].as_str().ok_or("Missing rpc")?;
            let objects = config.config["objects"]
                .as_array()
                .ok_or("Missing objects")?
                .iter()
                .map(|v| {
                    let query = v["query"].as_str().unwrap();
                    let keys = v["keys"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| {
                            let value_type = v["type"].as_str().unwrap();
                            let value_type = match value_type {
                                "int" => super::adapters::base::ValueType::Int,
                                "float" => super::adapters::base::ValueType::Float,
                                _ => panic!("Unknown value type: {}", value_type),
                            };
                            (
                                v["key"].as_str().unwrap(),
                                v["path"].as_str().unwrap(),
                                value_type,
                            )
                        })
                        .collect::<Vec<_>>();
                    (query, keys)
                })
                .collect::<Vec<_>>();

            let queries = objects
                .iter()
                .fold(HashMap::new(), |mut acc, (query, keys)| {
                    acc.insert(
                        query.to_string(),
                        keys.clone()
                            .iter()
                            .map(|(a, b, c)| (a.to_string(), b.to_string(), c.clone()))
                            .collect::<Vec<_>>(),
                    );
                    acc
                });

            Ok(Box::new(
                CosmWasmSmartQueryAdapter::new(&name, contract, metrics, queries, rpc_endpoint)
                    .await?,
            ))
        }
        _ => Err(format!("Unknown adapter: {}", config.adapter).into()),
    }
}
