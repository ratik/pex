use std::collections::HashMap;
use std::sync::Arc;

use super::base::MetricsAdapter;
use ethers::providers::Middleware;
use ethers::types::U256;
use tokio::sync::Mutex;

pub struct ETHAdapter {
    name: String,
    decimals: u8,
    addresses: Vec<String>,
    client: Arc<ethers::prelude::Provider<ethers::providers::Http>>,
}

#[async_trait::async_trait]
impl MetricsAdapter for ETHAdapter {
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

impl ETHAdapter {
    pub async fn new(
        name: &str,
        metrics: Arc<Mutex<HashMap<String, super::base::Value>>>,
        addresses: Vec<&str>,
        token: &str,
        decimals: u8,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let provider = ethers::prelude::Provider::<ethers::providers::Http>::try_from(
            "https://mainnet.infura.io/v3/".to_string() + token,
        )?;
        let client = Arc::new(provider);
        let mut storage = metrics.lock().await;
        for addr in addresses.clone() {
            let key = format!("{}_balance_{}", name, addr);
            let value = super::base::Value::Int(prometheus::IntGauge::new(
                &key,
                format!("Value of {}", key),
            )?);
            storage.insert(key, value);
        }

        Ok(Self {
            name: name.to_string(),
            decimals,
            client,
            addresses: addresses
                .clone()
                .iter()
                .map(|addr| addr.parse().unwrap())
                .collect(),
        })
    }

    async fn update_balances(
        &mut self,
        storage: &mut HashMap<String, super::base::Value>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for addr in self.addresses.clone() {
            let address: ethers::types::Address = addr.parse()?;
            let balance: U256 = self
                .client
                .get_balance(address, None)
                .await?
                .checked_div(U256::from(10).pow(self.decimals.into()))
                .unwrap();
            let key = "balance_".to_string() + &addr;
            match storage.get(&self.get_key(&key)) {
                Some(super::base::Value::Int(v)) => {
                    v.set(balance.as_u64() as i64);
                }
                _ => unreachable!(),
            }
        }
        Ok(())
    }
}
