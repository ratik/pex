use bytes::Bytes;
use cosmos_sdk_proto::cosmos::base::query::v1beta1::PageRequest;
use cosmos_sdk_proto::traits::Message;
use std::collections::HashMap;
use std::sync::Arc;
use std::vec;
use tendermint_rpc::Client;
use tokio::sync::Mutex;

use super::base::MetricsAdapter;

pub struct CosmosBankAdapter {
    name: String,
    addresses: Vec<String>,
    client: tendermint_rpc::HttpClient,
    denoms: Vec<String>,
}

#[async_trait::async_trait]
impl MetricsAdapter for CosmosBankAdapter {
    fn get_name(&self) -> &str {
        &self.name
    }

    async fn update_params(
        &mut self,
        metrics: Arc<Mutex<HashMap<String, super::base::Value>>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut storage = metrics.lock().await;
        self.update_balances(&mut storage).await?;
        Ok(())
    }
}

impl CosmosBankAdapter {
    pub async fn new(
        name: &str,
        metrics: Arc<Mutex<HashMap<String, super::base::Value>>>,
        addresses: Vec<&str>,
        rpc: &str,
        denoms: Vec<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let client = tendermint_rpc::HttpClient::builder(rpc.parse()?).build()?;

        let mut storage = metrics.lock().await;
        for addr in addresses.clone() {
            for denom in denoms.clone() {
                let key = format!("{}_balance_{}_{}", name, addr, denom);
                let value = super::base::Value::Int(prometheus::IntGauge::new(
                    &key,
                    format!("Value of {}", key),
                )?);
                storage.insert(key, value);
            }
        }

        Ok(Self {
            name: name.to_string(),
            addresses: addresses
                .clone()
                .iter()
                .map(|addr| addr.parse().unwrap())
                .collect(),
            client,
            denoms: denoms.iter().map(|d| d.to_string()).collect(),
        })
    }

    async fn update_balances(
        &mut self,
        storage: &mut HashMap<String, super::base::Value>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for address in self.addresses.clone() {
            let path = Some("/cosmos.bank.v1beta1.Query/AllBalances".to_string());
            let req = cosmos_sdk_proto::cosmos::bank::v1beta1::QueryAllBalancesRequest {
                address: address.clone(),
                pagination: Some(PageRequest {
                    key: vec![],
                    offset: 0u64,
                    limit: 1000u64,
                    count_total: false,
                    reverse: false,
                }),
                resolve_denom: false,
            };
            let mut buf = Vec::with_capacity(req.encoded_len());
            req.encode(&mut buf).unwrap();

            let answer = self.client.abci_query(path, buf, None, false).await?;
            let buf = answer.value;
            let balance =
                cosmos_sdk_proto::cosmos::bank::v1beta1::QueryAllBalancesResponse::decode(
                    Bytes::from(buf),
                )
                .unwrap();
            for b in balance.balances {
                let denom = b.denom;
                let amount = b.amount;
                if self.denoms.contains(&denom) {
                    let key = format!("balance_{}_{}", &address, denom);
                    let val = storage.get(&self.get_key(&key));
                    println!("{} - {:?}", self.get_key(&key), val);
                    match val {
                        Some(super::base::Value::Int(v)) => {
                            v.set(amount.parse()?);
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
        Ok(())
    }
}
