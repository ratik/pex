use super::base::MetricsAdapter;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct BTCAdapter {
    name: String,
    addresses: Vec<String>,
}

#[async_trait::async_trait]
impl MetricsAdapter for BTCAdapter {
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

impl BTCAdapter {
    pub async fn new(
        name: &str,
        metrics: Arc<Mutex<HashMap<String, super::base::Value>>>,
        addresses: Vec<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
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
            let url = format!("https://blockchain.info/q/addressbalance/{}", &addr);
            let key = "balance_".to_string() + &addr;
            let response = reqwest::get(&url).await?.text().await?;
            let balance_satoshis: u64 = response.trim().parse()?;
            match storage.get(&self.get_key(&key)) {
                Some(super::base::Value::Int(v)) => {
                    v.set(balance_satoshis as i64);
                }
                _ => unreachable!(),
            }
        }
        Ok(())
    }
}
