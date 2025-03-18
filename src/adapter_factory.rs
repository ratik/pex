use ethers::core::k256::elliptic_curve::rand_core::le;
use serde::de::value;
use tokio::sync::Mutex;

use crate::adapters::base::ValueType;
use crate::adapters::cosmwasm_smart_query::CosmWasmSmartQueryAdapter;
use crate::adapters::{
    base::MetricsAdapter, compound::CompoundAdapter, cosmos_bank::CosmosBankAdapter,
    erc20::Erc20Adapter,
};
use crate::config::MetricConfig;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

pub async fn create_adapter(
    name: String,
    metrics: Arc<Mutex<HashMap<String, super::adapters::base::Value>>>,
    config: &MetricConfig,
) -> Result<Box<dyn MetricsAdapter + Send + Sync>, Box<dyn Error>> {
    println!("Creating adapter: {}", config.adapter);
    match config.adapter.as_str() {
        "compound" | "erc20" => {
            let addresses = config.config["addresses"]
                .as_array()
                .ok_or("Missing addresses")?
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<Vec<_>>();

            let contract = config.config["contract"]
                .as_str()
                .ok_or("Missing contract")?;

            let infura_token = config.config["infura_token"]
                .as_str()
                .ok_or("Missing infura_token")?;

            let decimals: u8 = config.config["decimals"].as_u64().unwrap() as u8 - 2;

            match config.adapter.as_str() {
                "compound" => Ok(Box::new(
                    CompoundAdapter::new(
                        &name,
                        metrics,
                        addresses,
                        contract,
                        infura_token,
                        decimals,
                    )
                    .await?,
                )),
                "erc20" => Ok(Box::new(
                    Erc20Adapter::new(&name, metrics, addresses, contract, infura_token, decimals)
                        .await?,
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
