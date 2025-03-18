use bytes::Bytes;
use cosmos_sdk_proto::traits::Message;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tendermint_rpc::Client;
use tokio::sync::Mutex;

use super::base::{MetricsAdapter, ValueType};

pub type StorageKey = String;
pub type JqPath = String;
pub type SmartQuery = String;

pub struct CosmWasmSmartQueryAdapter {
    name: String,
    contract_address: String,
    queries: HashMap<SmartQuery, Vec<(StorageKey, JqPath, ValueType)>>,
    client: tendermint_rpc::HttpClient,
}

#[async_trait::async_trait]
impl MetricsAdapter for CosmWasmSmartQueryAdapter {
    fn get_name(&self) -> &str {
        &self.name
    }

    async fn update_params(
        &mut self,
        metrics: Arc<Mutex<HashMap<String, super::base::Value>>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let storage = metrics.lock().await;
        for (smart_query, items) in self.queries.iter() {
            let path = Some("/cosmwasm.wasm.v1.Query/SmartContractState".to_string());
            let req = cosmos_sdk_proto::cosmwasm::wasm::v1::QuerySmartContractStateRequest {
                address: self.contract_address.to_string(),
                query_data: smart_query.to_string().into(),
            };
            let mut buf = Vec::with_capacity(req.encoded_len());
            req.encode(&mut buf).unwrap();
            let answer = self.client.abci_query(path, buf, None, false).await?;
            let buf = answer.value;
            let response =
                cosmos_sdk_proto::cosmwasm::wasm::v1::QuerySmartContractStateResponse::decode(
                    Bytes::from(buf),
                )
                .unwrap();
            let json: Value = serde_json::from_slice(&response.data)?;
            for (key, jq_path, value_type) in items {
                let new_value: Value =
                    get_json_value_by_path(json.clone().into(), jq_path).unwrap();
                match value_type {
                    ValueType::Int => {
                        let key = self.get_key(&key);
                        let val = storage.get(&key);
                        match val {
                            Some(super::base::Value::Int(v)) => {
                                v.set(new_value.as_i64().unwrap());
                            }
                            _ => unreachable!(),
                        }
                    }
                    ValueType::Float => {
                        let key = self.get_key(&key);
                        let val = storage.get(&key);
                        match val {
                            Some(super::base::Value::Float(v)) => {
                                v.set(new_value.as_f64().unwrap());
                            }
                            _ => unreachable!(),
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl CosmWasmSmartQueryAdapter {
    pub async fn new(
        name: &str,
        contract_address: &str,
        metrics: Arc<Mutex<HashMap<String, super::base::Value>>>,
        queries: HashMap<SmartQuery, Vec<(StorageKey, JqPath, ValueType)>>,
        rpc: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let client = tendermint_rpc::HttpClient::builder(rpc.parse()?).build()?;

        let mut storage = metrics.lock().await;

        for (_, rows) in queries.iter() {
            for (key, _, value_type) in rows.iter() {
                let key = format!("{}_{}", name, key);
                match *value_type {
                    ValueType::Int => {
                        let value = super::base::Value::Int(prometheus::IntGauge::new(
                            key.clone(),
                            format!("Value of {}", key),
                        )?);
                        storage.insert(key.to_string(), value);
                    }
                    ValueType::Float => {
                        let value = super::base::Value::Float(prometheus::Gauge::new(
                            key.clone(),
                            format!("Value of {}", key),
                        )?);
                        storage.insert(key.to_string(), value);
                    }
                }
            }
        }

        Ok(Self {
            name: name.to_string(),
            contract_address: contract_address.to_string(),
            queries,
            client,
        })
    }
}

fn get_json_value_by_path(json: jaq_json::Val, path: &str) -> Option<Value> {
    use jaq_core::load::{Arena, File, Loader};

    let arena = Arena::default();
    let loader = Loader::new(jaq_std::defs().chain(jaq_json::defs()));
    let modules = loader
        .load(&arena, File {
            path: (),
            code: path,
        })
        .unwrap();
    let filter = jaq_core::Compiler::default()
        .with_funs(jaq_std::funs().chain(jaq_json::funs()))
        .compile(modules)
        .unwrap();
    let inputs = jaq_core::RcIter::new(core::iter::empty());
    let out = filter
        .run((jaq_core::Ctx::new([], &inputs), json))
        .collect::<Vec<_>>();
    if out.is_empty() {
        None
    } else {
        Some(out[0].clone().unwrap().into())
    }
}
