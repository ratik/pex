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
    addresses: Vec<String>,
    client: tendermint_rpc::HttpClient,
    denoms: Vec<String>,
    params: Vec<String>,
    values: Arc<Mutex<HashMap<String, super::base::Value>>>,
}

#[async_trait::async_trait]
impl MetricsAdapter for CosmosBankAdapter {
    fn get_params(&self) -> &Vec<String> {
        &self.params
    }

    fn get_values(&self) -> Arc<Mutex<HashMap<String, super::base::Value>>> {
        Arc::clone(&self.values)
    }

    async fn update_params(
        &mut self,
        _params: Option<Vec<String>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.update_balances().await?;
        Ok(())
    }
}

impl CosmosBankAdapter {
    pub fn new(
        addresses: Vec<&str>,
        rpc: &str,
        denoms: Vec<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let client = tendermint_rpc::HttpClient::builder(rpc.parse()?).build()?;
        let mut params = vec![];
        for addr in addresses.clone() {
            for denom in denoms.clone() {
                params.push(format!("balance_{}_{}", addr, denom));
            }
        }

        Ok(Self {
            addresses: addresses
                .clone()
                .iter()
                .map(|addr| addr.parse().unwrap())
                .collect(),
            client,
            params,
            denoms: denoms.iter().map(|d| d.to_string()).collect(),
            values: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    async fn update_balances(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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
                    let mut values = self.values.lock().await;
                    let key = format!("balance_{}_{}", &address, denom);
                    values.insert(key, super::base::Value::Int(amount.parse()?));
                }
            }
        }
        Ok(())
    }
}
