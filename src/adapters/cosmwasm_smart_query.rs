use bytes::Bytes;
use cosmos_sdk_proto::traits::Message;
use jaq_interpret::FilterT;
use jaq_interpret::Val;
use jaq_parse::parse;
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
            println!("{:?}", smart_query);
            let req = cosmos_sdk_proto::cosmwasm::wasm::v1::QuerySmartContractStateRequest {
                address: self.contract_address.to_string(),
                query_data: smart_query.to_string().into(),
            };
            println!("{:?}", req);
            let mut buf = Vec::with_capacity(req.encoded_len());
            req.encode(&mut buf).unwrap();

            let answer = self.client.abci_query(path, buf, None, false).await?;
            let buf = answer.value;
            let response =
                cosmos_sdk_proto::cosmwasm::wasm::v1::QuerySmartContractStateResponse::decode(
                    Bytes::from(buf),
                )
                .unwrap();
            println!("{:?}", response);
            let json = &serde_json::from_slice(&response.data)?;
            for (key, jq_path, value_type) in items {
                let new_value = get_json_value_by_path(json, jq_path).unwrap();
                let new_value = new_value.to_string();

                println!("!!!!{:?}", new_value);
                unimplemented!();
                match value_type {
                    ValueType::Int => {
                        let key = self.get_key(&key);
                        let val = storage.get(&key);
                        match val {
                            Some(super::base::Value::Int(v)) => {
                                v.set(new_value.parse()?);
                            }
                            _ => unreachable!(),
                        }
                    }
                    ValueType::Float => {
                        let key = self.get_key(&key);
                        let val = storage.get(&key);
                        match val {
                            Some(super::base::Value::Float(v)) => {
                                v.set(new_value.parse()?);
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

fn get_json_value_by_path(json_value: &Value, query_path: &str) -> Option<Value> {
    let value: Val = json_value.clone().into();
    let mut defs = jaq_interpret::ParseCtx::new(Vec::new());
    // defs.insert_natives(jaq_json::defs());
    // defs.insert_defs();

    let (main, errs) = parse(query_path, jaq_parse::main());
    if !errs.is_empty() {
        panic!("Parsing error(s) encountered");
    }
    let out: Vec<serde_json::Value> = defs
        .compile(main.unwrap())
        .run((
            jaq_interpret::Ctx::new([], &jaq_interpret::RcIter::new(core::iter::empty())),
            jaq_interpret::Val::from(value),
        ))
        .filter_map(|x| Some(x.ok()?.into()))
        .collect::<Vec<_>>();
    println!("{:?}", out);
    if out.is_empty() {
        None
    } else {
        Some(out[0].clone())
    }
}
