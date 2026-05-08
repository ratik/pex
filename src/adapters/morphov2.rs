use super::base::MetricsAdapter;
use ethers::abi::{Abi, ParamType, Token, decode, encode};
use ethers::types::U256;
use ethers::types::{Address, Bytes, U128};
use ethers::utils::hex::ToHexExt;
use ethers::utils::keccak256;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

fn mul_div_floor(a: U256, b: U256, c: U256) -> U256 {
    if c.is_zero() {
        return U256::zero();
    }
    a.saturating_mul(b) / c
}

type MarketParams = (Address, Address, Address, Address, U256);

fn market_id(params: &MarketParams) -> [u8; 32] {
    keccak256(encode(&[
        Token::Address(params.0),
        Token::Address(params.1),
        Token::Address(params.2),
        Token::Address(params.3),
        Token::Uint(params.4),
    ]))
}

fn decode_liquidity_market_params(data: &Bytes) -> Option<MarketParams> {
    if data.is_empty() {
        return None;
    }

    let tokens = decode(
        &[ParamType::Tuple(vec![
            ParamType::Address,
            ParamType::Address,
            ParamType::Address,
            ParamType::Address,
            ParamType::Uint(256),
        ])],
        data.as_ref(),
    )
    .ok()?;

    match tokens.into_iter().next()? {
        Token::Tuple(values) if values.len() == 5 => {
            let loan_token = values[0].clone().into_address()?;
            let collateral_token = values[1].clone().into_address()?;
            let oracle = values[2].clone().into_address()?;
            let irm = values[3].clone().into_address()?;
            let lltv = values[4].clone().into_uint()?;
            Some((loan_token, collateral_token, oracle, irm, lltv))
        }
        _ => None,
    }
}

pub struct MorphoV2Adapter {
    name: String,
    addresses: Vec<String>,
    token: String,
    contract: ethers::contract::ContractInstance<
        Arc<ethers::providers::Provider<ethers::providers::Http>>,
        ethers::providers::Provider<ethers::providers::Http>,
    >,
    la_contract: ethers::contract::ContractInstance<
        Arc<ethers::providers::Provider<ethers::providers::Http>>,
        ethers::providers::Provider<ethers::providers::Http>,
    >,
    main_contract: ethers::contract::ContractInstance<
        Arc<ethers::providers::Provider<ethers::providers::Http>>,
        ethers::providers::Provider<ethers::providers::Http>,
    >,
}

#[async_trait::async_trait]
impl MetricsAdapter for MorphoV2Adapter {
    fn get_name(&self) -> &str {
        &self.name
    }

    async fn update_params(
        &mut self,
        metrics: Arc<Mutex<HashMap<String, super::base::Value>>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut storage = metrics.lock().await;
        self.update_balances(&mut storage).await?;
        self.update_free_liquidity(&mut storage).await?;
        Ok(())
    }
}

impl MorphoV2Adapter {
    pub async fn new(
        name: &str,
        metrics: Arc<Mutex<HashMap<String, super::base::Value>>>,
        addresses: Vec<&str>,
        contract: &str,
        rpc: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let abi: Abi = serde_json::from_str(
            r#"[{"inputs":[{"internalType":"address","name":"_owner","type":"address"},{"internalType":"address","name":"_asset","type":"address"}],"stateMutability":"nonpayable","type":"constructor"},{"inputs":[],"name":"Abdicated","type":"error"},{"inputs":[],"name":"AbsoluteCapExceeded","type":"error"},{"inputs":[],"name":"AbsoluteCapNotDecreasing","type":"error"},{"inputs":[],"name":"AbsoluteCapNotIncreasing","type":"error"},{"inputs":[],"name":"AutomaticallyTimelocked","type":"error"},{"inputs":[],"name":"CannotReceiveAssets","type":"error"},{"inputs":[],"name":"CannotReceiveShares","type":"error"},{"inputs":[],"name":"CannotSendAssets","type":"error"},{"inputs":[],"name":"CannotSendShares","type":"error"},{"inputs":[],"name":"CastOverflow","type":"error"},{"inputs":[],"name":"DataAlreadyPending","type":"error"},{"inputs":[],"name":"DataNotTimelocked","type":"error"},{"inputs":[],"name":"FeeInvariantBroken","type":"error"},{"inputs":[],"name":"FeeTooHigh","type":"error"},{"inputs":[],"name":"InvalidSigner","type":"error"},{"inputs":[],"name":"MaxRateTooHigh","type":"error"},{"inputs":[],"name":"NoCode","type":"error"},{"inputs":[],"name":"NotAdapter","type":"error"},{"inputs":[],"name":"NotInAdapterRegistry","type":"error"},{"inputs":[],"name":"PenaltyTooHigh","type":"error"},{"inputs":[],"name":"PermitDeadlineExpired","type":"error"},{"inputs":[],"name":"RelativeCapAboveOne","type":"error"},{"inputs":[],"name":"RelativeCapExceeded","type":"error"},{"inputs":[],"name":"RelativeCapNotDecreasing","type":"error"},{"inputs":[],"name":"RelativeCapNotIncreasing","type":"error"},{"inputs":[],"name":"TimelockNotDecreasing","type":"error"},{"inputs":[],"name":"TimelockNotExpired","type":"error"},{"inputs":[],"name":"TimelockNotIncreasing","type":"error"},{"inputs":[],"name":"TransferFromReturnedFalse","type":"error"},{"inputs":[],"name":"TransferFromReverted","type":"error"},{"inputs":[],"name":"TransferReturnedFalse","type":"error"},{"inputs":[],"name":"TransferReverted","type":"error"},{"inputs":[],"name":"Unauthorized","type":"error"},{"inputs":[],"name":"ZeroAbsoluteCap","type":"error"},{"inputs":[],"name":"ZeroAddress","type":"error"},{"inputs":[],"name":"ZeroAllocation","type":"error"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes4","name":"selector","type":"bytes4"}],"name":"Abdicate","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes4","name":"selector","type":"bytes4"},{"indexed":false,"internalType":"bytes","name":"data","type":"bytes"}],"name":"Accept","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"internalType":"uint256","name":"previousTotalAssets","type":"uint256"},{"indexed":false,"internalType":"uint256","name":"newTotalAssets","type":"uint256"},{"indexed":false,"internalType":"uint256","name":"performanceFeeShares","type":"uint256"},{"indexed":false,"internalType":"uint256","name":"managementFeeShares","type":"uint256"}],"name":"AccrueInterest","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"account","type":"address"}],"name":"AddAdapter","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"sender","type":"address"},{"indexed":true,"internalType":"address","name":"adapter","type":"address"},{"indexed":false,"internalType":"uint256","name":"assets","type":"uint256"},{"indexed":false,"internalType":"bytes32[]","name":"ids","type":"bytes32[]"},{"indexed":false,"internalType":"int256","name":"change","type":"int256"}],"name":"Allocate","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"owner","type":"address"},{"indexed":true,"internalType":"address","name":"spender","type":"address"},{"indexed":false,"internalType":"uint256","name":"shares","type":"uint256"}],"name":"AllowanceUpdatedByTransferFrom","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"owner","type":"address"},{"indexed":true,"internalType":"address","name":"spender","type":"address"},{"indexed":false,"internalType":"uint256","name":"shares","type":"uint256"}],"name":"Approval","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"owner","type":"address"},{"indexed":true,"internalType":"address","name":"asset","type":"address"}],"name":"Constructor","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"sender","type":"address"},{"indexed":true,"internalType":"address","name":"adapter","type":"address"},{"indexed":false,"internalType":"uint256","name":"assets","type":"uint256"},{"indexed":false,"internalType":"bytes32[]","name":"ids","type":"bytes32[]"},{"indexed":false,"internalType":"int256","name":"change","type":"int256"}],"name":"Deallocate","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"sender","type":"address"},{"indexed":true,"internalType":"bytes32","name":"id","type":"bytes32"},{"indexed":false,"internalType":"bytes","name":"idData","type":"bytes"},{"indexed":false,"internalType":"uint256","name":"newAbsoluteCap","type":"uint256"}],"name":"DecreaseAbsoluteCap","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"sender","type":"address"},{"indexed":true,"internalType":"bytes32","name":"id","type":"bytes32"},{"indexed":false,"internalType":"bytes","name":"idData","type":"bytes"},{"indexed":false,"internalType":"uint256","name":"newRelativeCap","type":"uint256"}],"name":"DecreaseRelativeCap","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes4","name":"selector","type":"bytes4"},{"indexed":false,"internalType":"uint256","name":"newDuration","type":"uint256"}],"name":"DecreaseTimelock","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"sender","type":"address"},{"indexed":true,"internalType":"address","name":"onBehalf","type":"address"},{"indexed":false,"internalType":"uint256","name":"assets","type":"uint256"},{"indexed":false,"internalType":"uint256","name":"shares","type":"uint256"}],"name":"Deposit","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"sender","type":"address"},{"indexed":false,"internalType":"address","name":"adapter","type":"address"},{"indexed":false,"internalType":"uint256","name":"assets","type":"uint256"},{"indexed":true,"internalType":"address","name":"onBehalf","type":"address"},{"indexed":false,"internalType":"bytes32[]","name":"ids","type":"bytes32[]"},{"indexed":false,"internalType":"uint256","name":"penaltyAssets","type":"uint256"}],"name":"ForceDeallocate","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes32","name":"id","type":"bytes32"},{"indexed":false,"internalType":"bytes","name":"idData","type":"bytes"},{"indexed":false,"internalType":"uint256","name":"newAbsoluteCap","type":"uint256"}],"name":"IncreaseAbsoluteCap","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes32","name":"id","type":"bytes32"},{"indexed":false,"internalType":"bytes","name":"idData","type":"bytes"},{"indexed":false,"internalType":"uint256","name":"newRelativeCap","type":"uint256"}],"name":"IncreaseRelativeCap","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes4","name":"selector","type":"bytes4"},{"indexed":false,"internalType":"uint256","name":"newDuration","type":"uint256"}],"name":"IncreaseTimelock","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"owner","type":"address"},{"indexed":true,"internalType":"address","name":"spender","type":"address"},{"indexed":false,"internalType":"uint256","name":"shares","type":"uint256"},{"indexed":false,"internalType":"uint256","name":"nonce","type":"uint256"},{"indexed":false,"internalType":"uint256","name":"deadline","type":"uint256"}],"name":"Permit","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"account","type":"address"}],"name":"RemoveAdapter","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"sender","type":"address"},{"indexed":true,"internalType":"bytes4","name":"selector","type":"bytes4"},{"indexed":false,"internalType":"bytes","name":"data","type":"bytes"}],"name":"Revoke","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"newAdapterRegistry","type":"address"}],"name":"SetAdapterRegistry","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"newCurator","type":"address"}],"name":"SetCurator","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"adapter","type":"address"},{"indexed":false,"internalType":"uint256","name":"forceDeallocatePenalty","type":"uint256"}],"name":"SetForceDeallocatePenalty","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"account","type":"address"},{"indexed":false,"internalType":"bool","name":"newIsAllocator","type":"bool"}],"name":"SetIsAllocator","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"account","type":"address"},{"indexed":false,"internalType":"bool","name":"newIsSentinel","type":"bool"}],"name":"SetIsSentinel","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"sender","type":"address"},{"indexed":true,"internalType":"address","name":"newLiquidityAdapter","type":"address"},{"indexed":true,"internalType":"bytes","name":"newLiquidityData","type":"bytes"}],"name":"SetLiquidityAdapterAndData","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"internalType":"uint256","name":"newManagementFee","type":"uint256"}],"name":"SetManagementFee","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"newManagementFeeRecipient","type":"address"}],"name":"SetManagementFeeRecipient","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"internalType":"uint256","name":"newMaxRate","type":"uint256"}],"name":"SetMaxRate","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"internalType":"string","name":"newName","type":"string"}],"name":"SetName","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"newOwner","type":"address"}],"name":"SetOwner","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"internalType":"uint256","name":"newPerformanceFee","type":"uint256"}],"name":"SetPerformanceFee","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"newPerformanceFeeRecipient","type":"address"}],"name":"SetPerformanceFeeRecipient","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"newReceiveAssetsGate","type":"address"}],"name":"SetReceiveAssetsGate","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"newReceiveSharesGate","type":"address"}],"name":"SetReceiveSharesGate","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"newSendAssetsGate","type":"address"}],"name":"SetSendAssetsGate","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"newSendSharesGate","type":"address"}],"name":"SetSendSharesGate","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"internalType":"string","name":"newSymbol","type":"string"}],"name":"SetSymbol","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes4","name":"selector","type":"bytes4"},{"indexed":false,"internalType":"bytes","name":"data","type":"bytes"},{"indexed":false,"internalType":"uint256","name":"executableAt","type":"uint256"}],"name":"Submit","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"from","type":"address"},{"indexed":true,"internalType":"address","name":"to","type":"address"},{"indexed":false,"internalType":"uint256","name":"shares","type":"uint256"}],"name":"Transfer","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"sender","type":"address"},{"indexed":true,"internalType":"address","name":"receiver","type":"address"},{"indexed":true,"internalType":"address","name":"onBehalf","type":"address"},{"indexed":false,"internalType":"uint256","name":"assets","type":"uint256"},{"indexed":false,"internalType":"uint256","name":"shares","type":"uint256"}],"name":"Withdraw","type":"event"},{"inputs":[],"name":"DOMAIN_SEPARATOR","outputs":[{"internalType":"bytes32","name":"","type":"bytes32"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"_totalAssets","outputs":[{"internalType":"uint128","name":"","type":"uint128"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"bytes4","name":"selector","type":"bytes4"}],"name":"abdicate","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes4","name":"selector","type":"bytes4"}],"name":"abdicated","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"bytes32","name":"id","type":"bytes32"}],"name":"absoluteCap","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"accrueInterest","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[],"name":"accrueInterestView","outputs":[{"internalType":"uint256","name":"","type":"uint256"},{"internalType":"uint256","name":"","type":"uint256"},{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"adapterRegistry","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"uint256","name":"","type":"uint256"}],"name":"adapters","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"adaptersLength","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"account","type":"address"}],"name":"addAdapter","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"adapter","type":"address"},{"internalType":"bytes","name":"data","type":"bytes"},{"internalType":"uint256","name":"assets","type":"uint256"}],"name":"allocate","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes32","name":"id","type":"bytes32"}],"name":"allocation","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"owner","type":"address"},{"internalType":"address","name":"spender","type":"address"}],"name":"allowance","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"spender","type":"address"},{"internalType":"uint256","name":"shares","type":"uint256"}],"name":"approve","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[],"name":"asset","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"account","type":"address"}],"name":"balanceOf","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"account","type":"address"}],"name":"canReceiveAssets","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"account","type":"address"}],"name":"canReceiveShares","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"account","type":"address"}],"name":"canSendAssets","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"account","type":"address"}],"name":"canSendShares","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"uint256","name":"shares","type":"uint256"}],"name":"convertToAssets","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"uint256","name":"assets","type":"uint256"}],"name":"convertToShares","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"curator","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"adapter","type":"address"},{"internalType":"bytes","name":"data","type":"bytes"},{"internalType":"uint256","name":"assets","type":"uint256"}],"name":"deallocate","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[],"name":"decimals","outputs":[{"internalType":"uint8","name":"","type":"uint8"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"bytes","name":"idData","type":"bytes"},{"internalType":"uint256","name":"newAbsoluteCap","type":"uint256"}],"name":"decreaseAbsoluteCap","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes","name":"idData","type":"bytes"},{"internalType":"uint256","name":"newRelativeCap","type":"uint256"}],"name":"decreaseRelativeCap","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes4","name":"selector","type":"bytes4"},{"internalType":"uint256","name":"newDuration","type":"uint256"}],"name":"decreaseTimelock","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"uint256","name":"assets","type":"uint256"},{"internalType":"address","name":"onBehalf","type":"address"}],"name":"deposit","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes","name":"data","type":"bytes"}],"name":"executableAt","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"firstTotalAssets","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"adapter","type":"address"},{"internalType":"bytes","name":"data","type":"bytes"},{"internalType":"uint256","name":"assets","type":"uint256"},{"internalType":"address","name":"onBehalf","type":"address"}],"name":"forceDeallocate","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"adapter","type":"address"}],"name":"forceDeallocatePenalty","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"bytes","name":"idData","type":"bytes"},{"internalType":"uint256","name":"newAbsoluteCap","type":"uint256"}],"name":"increaseAbsoluteCap","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes","name":"idData","type":"bytes"},{"internalType":"uint256","name":"newRelativeCap","type":"uint256"}],"name":"increaseRelativeCap","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes4","name":"selector","type":"bytes4"},{"internalType":"uint256","name":"newDuration","type":"uint256"}],"name":"increaseTimelock","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"account","type":"address"}],"name":"isAdapter","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"account","type":"address"}],"name":"isAllocator","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"account","type":"address"}],"name":"isSentinel","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"lastUpdate","outputs":[{"internalType":"uint64","name":"","type":"uint64"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"liquidityAdapter","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"liquidityData","outputs":[{"internalType":"bytes","name":"","type":"bytes"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"managementFee","outputs":[{"internalType":"uint96","name":"","type":"uint96"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"managementFeeRecipient","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"","type":"address"}],"name":"maxDeposit","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"pure","type":"function"},{"inputs":[{"internalType":"address","name":"","type":"address"}],"name":"maxMint","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"pure","type":"function"},{"inputs":[],"name":"maxRate","outputs":[{"internalType":"uint64","name":"","type":"uint64"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"","type":"address"}],"name":"maxRedeem","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"pure","type":"function"},{"inputs":[{"internalType":"address","name":"","type":"address"}],"name":"maxWithdraw","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"pure","type":"function"},{"inputs":[{"internalType":"uint256","name":"shares","type":"uint256"},{"internalType":"address","name":"onBehalf","type":"address"}],"name":"mint","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes[]","name":"data","type":"bytes[]"}],"name":"multicall","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[],"name":"name","outputs":[{"internalType":"string","name":"","type":"string"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"account","type":"address"}],"name":"nonces","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"owner","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"performanceFee","outputs":[{"internalType":"uint96","name":"","type":"uint96"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"performanceFeeRecipient","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"_owner","type":"address"},{"internalType":"address","name":"spender","type":"address"},{"internalType":"uint256","name":"shares","type":"uint256"},{"internalType":"uint256","name":"deadline","type":"uint256"},{"internalType":"uint8","name":"v","type":"uint8"},{"internalType":"bytes32","name":"r","type":"bytes32"},{"internalType":"bytes32","name":"s","type":"bytes32"}],"name":"permit","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"uint256","name":"assets","type":"uint256"}],"name":"previewDeposit","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"uint256","name":"shares","type":"uint256"}],"name":"previewMint","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"uint256","name":"shares","type":"uint256"}],"name":"previewRedeem","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"uint256","name":"assets","type":"uint256"}],"name":"previewWithdraw","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"receiveAssetsGate","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"receiveSharesGate","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"uint256","name":"shares","type":"uint256"},{"internalType":"address","name":"receiver","type":"address"},{"internalType":"address","name":"onBehalf","type":"address"}],"name":"redeem","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes32","name":"id","type":"bytes32"}],"name":"relativeCap","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"account","type":"address"}],"name":"removeAdapter","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes","name":"data","type":"bytes"}],"name":"revoke","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[],"name":"sendAssetsGate","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"sendSharesGate","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"newAdapterRegistry","type":"address"}],"name":"setAdapterRegistry","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"newCurator","type":"address"}],"name":"setCurator","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"adapter","type":"address"},{"internalType":"uint256","name":"newForceDeallocatePenalty","type":"uint256"}],"name":"setForceDeallocatePenalty","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"account","type":"address"},{"internalType":"bool","name":"newIsAllocator","type":"bool"}],"name":"setIsAllocator","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"account","type":"address"},{"internalType":"bool","name":"newIsSentinel","type":"bool"}],"name":"setIsSentinel","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"newLiquidityAdapter","type":"address"},{"internalType":"bytes","name":"newLiquidityData","type":"bytes"}],"name":"setLiquidityAdapterAndData","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"uint256","name":"newManagementFee","type":"uint256"}],"name":"setManagementFee","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"newManagementFeeRecipient","type":"address"}],"name":"setManagementFeeRecipient","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"uint256","name":"newMaxRate","type":"uint256"}],"name":"setMaxRate","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"string","name":"newName","type":"string"}],"name":"setName","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"newOwner","type":"address"}],"name":"setOwner","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"uint256","name":"newPerformanceFee","type":"uint256"}],"name":"setPerformanceFee","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"newPerformanceFeeRecipient","type":"address"}],"name":"setPerformanceFeeRecipient","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"newReceiveAssetsGate","type":"address"}],"name":"setReceiveAssetsGate","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"newReceiveSharesGate","type":"address"}],"name":"setReceiveSharesGate","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"newSendAssetsGate","type":"address"}],"name":"setSendAssetsGate","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"newSendSharesGate","type":"address"}],"name":"setSendSharesGate","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"string","name":"newSymbol","type":"string"}],"name":"setSymbol","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes","name":"data","type":"bytes"}],"name":"submit","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[],"name":"symbol","outputs":[{"internalType":"string","name":"","type":"string"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"bytes4","name":"selector","type":"bytes4"}],"name":"timelock","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"totalAssets","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"totalSupply","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"to","type":"address"},{"internalType":"uint256","name":"shares","type":"uint256"}],"name":"transfer","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"from","type":"address"},{"internalType":"address","name":"to","type":"address"},{"internalType":"uint256","name":"shares","type":"uint256"}],"name":"transferFrom","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[],"name":"virtualShares","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"uint256","name":"assets","type":"uint256"},{"internalType":"address","name":"receiver","type":"address"},{"internalType":"address","name":"onBehalf","type":"address"}],"name":"withdraw","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"nonpayable","type":"function"}]"#,
        )?;

        let la_abi: Abi = serde_json::from_str(
            r#"[{"inputs":[{"internalType":"address","name":"_parentVault","type":"address"},{"internalType":"address","name":"_morpho","type":"address"},{"internalType":"address","name":"_adaptiveCurveIrm","type":"address"}],"stateMutability":"nonpayable","type":"constructor"},{"inputs":[],"name":"Abdicated","type":"error"},{"inputs":[],"name":"ApproveReturnedFalse","type":"error"},{"inputs":[],"name":"ApproveReverted","type":"error"},{"inputs":[],"name":"AutomaticallyTimelocked","type":"error"},{"inputs":[],"name":"DataAlreadyPending","type":"error"},{"inputs":[],"name":"DataNotTimelocked","type":"error"},{"inputs":[],"name":"IrmMismatch","type":"error"},{"inputs":[],"name":"LoanAssetMismatch","type":"error"},{"inputs":[],"name":"NoCode","type":"error"},{"inputs":[],"name":"SharePriceAboveOne","type":"error"},{"inputs":[],"name":"TimelockNotDecreasing","type":"error"},{"inputs":[],"name":"TimelockNotExpired","type":"error"},{"inputs":[],"name":"TimelockNotIncreasing","type":"error"},{"inputs":[],"name":"TransferReturnedFalse","type":"error"},{"inputs":[],"name":"TransferReverted","type":"error"},{"inputs":[],"name":"Unauthorized","type":"error"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes4","name":"selector","type":"bytes4"}],"name":"Abdicate","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes4","name":"selector","type":"bytes4"},{"indexed":false,"internalType":"bytes","name":"data","type":"bytes"}],"name":"Accept","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes32","name":"marketId","type":"bytes32"},{"indexed":false,"internalType":"uint256","name":"newAllocation","type":"uint256"},{"indexed":false,"internalType":"uint256","name":"mintedShares","type":"uint256"}],"name":"Allocate","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes32","name":"marketId","type":"bytes32"},{"indexed":false,"internalType":"uint256","name":"supplyShares","type":"uint256"}],"name":"BurnShares","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes32","name":"marketId","type":"bytes32"},{"indexed":false,"internalType":"uint256","name":"newAllocation","type":"uint256"},{"indexed":false,"internalType":"uint256","name":"burnedShares","type":"uint256"}],"name":"Deallocate","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes4","name":"selector","type":"bytes4"},{"indexed":false,"internalType":"uint256","name":"newDuration","type":"uint256"}],"name":"DecreaseTimelock","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes4","name":"selector","type":"bytes4"},{"indexed":false,"internalType":"uint256","name":"newDuration","type":"uint256"}],"name":"IncreaseTimelock","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"sender","type":"address"},{"indexed":true,"internalType":"bytes4","name":"selector","type":"bytes4"},{"indexed":false,"internalType":"bytes","name":"data","type":"bytes"}],"name":"Revoke","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"newSkimRecipient","type":"address"}],"name":"SetSkimRecipient","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"token","type":"address"},{"indexed":false,"internalType":"uint256","name":"assets","type":"uint256"}],"name":"Skim","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes4","name":"selector","type":"bytes4"},{"indexed":false,"internalType":"bytes","name":"data","type":"bytes"},{"indexed":false,"internalType":"uint256","name":"executableAt","type":"uint256"}],"name":"Submit","type":"event"},{"inputs":[{"internalType":"bytes4","name":"selector","type":"bytes4"}],"name":"abdicate","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes4","name":"selector","type":"bytes4"}],"name":"abdicated","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"adapterId","outputs":[{"internalType":"bytes32","name":"","type":"bytes32"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"adaptiveCurveIrm","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"bytes","name":"data","type":"bytes"},{"internalType":"uint256","name":"assets","type":"uint256"},{"internalType":"bytes4","name":"","type":"bytes4"},{"internalType":"address","name":"","type":"address"}],"name":"allocate","outputs":[{"internalType":"bytes32[]","name":"","type":"bytes32[]"},{"internalType":"int256","name":"","type":"int256"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"components":[{"internalType":"address","name":"loanToken","type":"address"},{"internalType":"address","name":"collateralToken","type":"address"},{"internalType":"address","name":"oracle","type":"address"},{"internalType":"address","name":"irm","type":"address"},{"internalType":"uint256","name":"lltv","type":"uint256"}],"internalType":"struct MarketParams","name":"marketParams","type":"tuple"}],"name":"allocation","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"asset","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"bytes32","name":"marketId","type":"bytes32"}],"name":"burnShares","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes","name":"data","type":"bytes"},{"internalType":"uint256","name":"assets","type":"uint256"},{"internalType":"bytes4","name":"","type":"bytes4"},{"internalType":"address","name":"","type":"address"}],"name":"deallocate","outputs":[{"internalType":"bytes32[]","name":"","type":"bytes32[]"},{"internalType":"int256","name":"","type":"int256"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes4","name":"selector","type":"bytes4"},{"internalType":"uint256","name":"newDuration","type":"uint256"}],"name":"decreaseTimelock","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes","name":"data","type":"bytes"}],"name":"executableAt","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"bytes32","name":"marketId","type":"bytes32"}],"name":"expectedSupplyAssets","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"factory","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[{"components":[{"internalType":"address","name":"loanToken","type":"address"},{"internalType":"address","name":"collateralToken","type":"address"},{"internalType":"address","name":"oracle","type":"address"},{"internalType":"address","name":"irm","type":"address"},{"internalType":"uint256","name":"lltv","type":"uint256"}],"internalType":"struct MarketParams","name":"marketParams","type":"tuple"}],"name":"ids","outputs":[{"internalType":"bytes32[]","name":"","type":"bytes32[]"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"bytes4","name":"selector","type":"bytes4"},{"internalType":"uint256","name":"newDuration","type":"uint256"}],"name":"increaseTimelock","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"uint256","name":"","type":"uint256"}],"name":"marketIds","outputs":[{"internalType":"bytes32","name":"","type":"bytes32"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"marketIdsLength","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"morpho","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"parentVault","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"realAssets","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"bytes","name":"data","type":"bytes"}],"name":"revoke","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"newSkimRecipient","type":"address"}],"name":"setSkimRecipient","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"token","type":"address"}],"name":"skim","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[],"name":"skimRecipient","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"bytes","name":"data","type":"bytes"}],"name":"submit","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes32","name":"marketId","type":"bytes32"}],"name":"supplyShares","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"bytes4","name":"selector","type":"bytes4"}],"name":"timelock","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"}]"#,
        )?;

        let main_abi: Abi = serde_json::from_str(
            r#"
            [{"inputs": [{"internalType": "address","name": "newOwner","type": "address"}],"stateMutability": "nonpayable","type": "constructor"},{"anonymous": false,"inputs": [{"indexed": true,"internalType": "Id","name": "id","type": "bytes32"},{"indexed": false,"internalType": "uint256","name": "prevBorrowRate","type": "uint256"},{"indexed": false,"internalType": "uint256","name": "interest","type": "uint256"},{"indexed": false,"internalType": "uint256","name": "feeShares","type": "uint256"}],"name": "AccrueInterest","type": "event"},{"anonymous": false,"inputs": [{"indexed": true,"internalType": "Id","name": "id","type": "bytes32"},{"indexed": false,"internalType": "address","name": "caller","type": "address"},{"indexed": true,"internalType": "address","name": "onBehalf","type": "address"},{"indexed": true,"internalType": "address","name": "receiver","type": "address"},{"indexed": false,"internalType": "uint256","name": "assets","type": "uint256"},{"indexed": false,"internalType": "uint256","name": "shares","type": "uint256"}],"name": "Borrow","type": "event"},{"anonymous": false,"inputs": [{"indexed": true,"internalType": "Id","name": "id","type": "bytes32"},{"components": [{"internalType": "address","name": "loanToken","type": "address"},{"internalType": "address","name": "collateralToken","type": "address"},{"internalType": "address","name": "oracle","type": "address"},{"internalType": "address","name": "irm","type": "address"},{"internalType": "uint256","name": "lltv","type": "uint256"}],"indexed": false,"internalType": "struct MarketParams","name": "marketParams","type": "tuple"}],"name": "CreateMarket","type": "event"},{"anonymous": false,"inputs": [{"indexed": true,"internalType": "address","name": "irm","type": "address"}],"name": "EnableIrm","type": "event"},{"anonymous": false,"inputs": [{"indexed": false,"internalType": "uint256","name": "lltv","type": "uint256"}],"name": "EnableLltv","type": "event"},{"anonymous": false,"inputs": [{"indexed": true,"internalType": "address","name": "caller","type": "address"},{"indexed": true,"internalType": "address","name": "token","type": "address"},{"indexed": false,"internalType": "uint256","name": "assets","type": "uint256"}],"name": "FlashLoan","type": "event"},{"anonymous": false,"inputs": [{"indexed": true,"internalType": "address","name": "caller","type": "address"},{"indexed": true,"internalType": "address","name": "authorizer","type": "address"},{"indexed": false,"internalType": "uint256","name": "usedNonce","type": "uint256"}],"name": "IncrementNonce","type": "event"},{"anonymous": false,"inputs": [{"indexed": true,"internalType": "Id","name": "id","type": "bytes32"},{"indexed": true,"internalType": "address","name": "caller","type": "address"},{"indexed": true,"internalType": "address","name": "borrower","type": "address"},{"indexed": false,"internalType": "uint256","name": "repaidAssets","type": "uint256"},{"indexed": false,"internalType": "uint256","name": "repaidShares","type": "uint256"},{"indexed": false,"internalType": "uint256","name": "seizedAssets","type": "uint256"},{"indexed": false,"internalType": "uint256","name": "badDebtAssets","type": "uint256"},{"indexed": false,"internalType": "uint256","name": "badDebtShares","type": "uint256"}],"name": "Liquidate","type": "event"},{"anonymous": false,"inputs": [{"indexed": true,"internalType": "Id","name": "id","type": "bytes32"},{"indexed": true,"internalType": "address","name": "caller","type": "address"},{"indexed": true,"internalType": "address","name": "onBehalf","type": "address"},{"indexed": false,"internalType": "uint256","name": "assets","type": "uint256"},{"indexed": false,"internalType": "uint256","name": "shares","type": "uint256"}],"name": "Repay","type": "event"},{"anonymous": false,"inputs": [{"indexed": true,"internalType": "address","name": "caller","type": "address"},{"indexed": true,"internalType": "address","name": "authorizer","type": "address"},{"indexed": true,"internalType": "address","name": "authorized","type": "address"},{"indexed": false,"internalType": "bool","name": "newIsAuthorized","type": "bool"}],"name": "SetAuthorization","type": "event"},{"anonymous": false,"inputs": [{"indexed": true,"internalType": "Id","name": "id","type": "bytes32"},{"indexed": false,"internalType": "uint256","name": "newFee","type": "uint256"}],"name": "SetFee","type": "event"},{"anonymous": false,"inputs": [{"indexed": true,"internalType": "address","name": "newFeeRecipient","type": "address"}],"name": "SetFeeRecipient","type": "event"},{"anonymous": false,"inputs": [{"indexed": true,"internalType": "address","name": "newOwner","type": "address"}],"name": "SetOwner","type": "event"},{"anonymous": false,"inputs": [{"indexed": true,"internalType": "Id","name": "id","type": "bytes32"},{"indexed": true,"internalType": "address","name": "caller","type": "address"},{"indexed": true,"internalType": "address","name": "onBehalf","type": "address"},{"indexed": false,"internalType": "uint256","name": "assets","type": "uint256"},{"indexed": false,"internalType": "uint256","name": "shares","type": "uint256"}],"name": "Supply","type": "event"},{"anonymous": false,"inputs": [{"indexed": true,"internalType": "Id","name": "id","type": "bytes32"},{"indexed": true,"internalType": "address","name": "caller","type": "address"},{"indexed": true,"internalType": "address","name": "onBehalf","type": "address"},{"indexed": false,"internalType": "uint256","name": "assets","type": "uint256"}],"name": "SupplyCollateral","type": "event"},{"anonymous": false,"inputs": [{"indexed": true,"internalType": "Id","name": "id","type": "bytes32"},{"indexed": false,"internalType": "address","name": "caller","type": "address"},{"indexed": true,"internalType": "address","name": "onBehalf","type": "address"},{"indexed": true,"internalType": "address","name": "receiver","type": "address"},{"indexed": false,"internalType": "uint256","name": "assets","type": "uint256"},{"indexed": false,"internalType": "uint256","name": "shares","type": "uint256"}],"name": "Withdraw","type": "event"},{"anonymous": false,"inputs": [{"indexed": true,"internalType": "Id","name": "id","type": "bytes32"},{"indexed": false,"internalType": "address","name": "caller","type": "address"},{"indexed": true,"internalType": "address","name": "onBehalf","type": "address"},{"indexed": true,"internalType": "address","name": "receiver","type": "address"},{"indexed": false,"internalType": "uint256","name": "assets","type": "uint256"}],"name": "WithdrawCollateral","type": "event"},{"inputs": [],"name": "DOMAIN_SEPARATOR","outputs": [{"internalType": "bytes32","name": "","type": "bytes32"}],"stateMutability": "view","type": "function"},{"inputs": [{"components": [{"internalType": "address","name": "loanToken","type": "address"},{"internalType": "address","name": "collateralToken","type": "address"},{"internalType": "address","name": "oracle","type": "address"},{"internalType": "address","name": "irm","type": "address"},{"internalType": "uint256","name": "lltv","type": "uint256"}],"internalType": "struct MarketParams","name": "marketParams","type": "tuple"}],"name": "accrueInterest","outputs": [],"stateMutability": "nonpayable","type": "function"},{"inputs": [{"components": [{"internalType": "address","name": "loanToken","type": "address"},{"internalType": "address","name": "collateralToken","type": "address"},{"internalType": "address","name": "oracle","type": "address"},{"internalType": "address","name": "irm","type": "address"},{"internalType": "uint256","name": "lltv","type": "uint256"}],"internalType": "struct MarketParams","name": "marketParams","type": "tuple"},{"internalType": "uint256","name": "assets","type": "uint256"},{"internalType": "uint256","name": "shares","type": "uint256"},{"internalType": "address","name": "onBehalf","type": "address"},{"internalType": "address","name": "receiver","type": "address"}],"name": "borrow","outputs": [{"internalType": "uint256","name": "","type": "uint256"},{"internalType": "uint256","name": "","type": "uint256"}],"stateMutability": "nonpayable","type": "function"},{"inputs": [{"components": [{"internalType": "address","name": "loanToken","type": "address"},{"internalType": "address","name": "collateralToken","type": "address"},{"internalType": "address","name": "oracle","type": "address"},{"internalType": "address","name": "irm","type": "address"},{"internalType": "uint256","name": "lltv","type": "uint256"}],"internalType": "struct MarketParams","name": "marketParams","type": "tuple"}],"name": "createMarket","outputs": [],"stateMutability": "nonpayable","type": "function"},{"inputs": [{"internalType": "address","name": "irm","type": "address"}],"name": "enableIrm","outputs": [],"stateMutability": "nonpayable","type": "function"},{"inputs": [{"internalType": "uint256","name": "lltv","type": "uint256"}],"name": "enableLltv","outputs": [],"stateMutability": "nonpayable","type": "function"},{"inputs": [{"internalType": "bytes32[]","name": "slots","type": "bytes32[]"}],"name": "extSloads","outputs": [{"internalType": "bytes32[]","name": "res","type": "bytes32[]"}],"stateMutability": "view","type": "function"},{"inputs": [],"name": "feeRecipient","outputs": [{"internalType": "address","name": "","type": "address"}],"stateMutability": "view","type": "function"},{"inputs": [{"internalType": "address","name": "token","type": "address"},{"internalType": "uint256","name": "assets","type": "uint256"},{"internalType": "bytes","name": "data","type": "bytes"}],"name": "flashLoan","outputs": [],"stateMutability": "nonpayable","type": "function"},{"inputs": [{"internalType": "Id","name": "","type": "bytes32"}],"name": "idToMarketParams","outputs": [{"internalType": "address","name": "loanToken","type": "address"},{"internalType": "address","name": "collateralToken","type": "address"},{"internalType": "address","name": "oracle","type": "address"},{"internalType": "address","name": "irm","type": "address"},{"internalType": "uint256","name": "lltv","type": "uint256"}],"stateMutability": "view","type": "function"},{"inputs": [{"internalType": "address","name": "","type": "address"},{"internalType": "address","name": "","type": "address"}],"name": "isAuthorized","outputs": [{"internalType": "bool","name": "","type": "bool"}],"stateMutability": "view","type": "function"},{"inputs": [{"internalType": "address","name": "","type": "address"}],"name": "isIrmEnabled","outputs": [{"internalType": "bool","name": "","type": "bool"}],"stateMutability": "view","type": "function"},{"inputs": [{"internalType": "uint256","name": "","type": "uint256"}],"name": "isLltvEnabled","outputs": [{"internalType": "bool","name": "","type": "bool"}],"stateMutability": "view","type": "function"},{"inputs": [{"components": [{"internalType": "address","name": "loanToken","type": "address"},{"internalType": "address","name": "collateralToken","type": "address"},{"internalType": "address","name": "oracle","type": "address"},{"internalType": "address","name": "irm","type": "address"},{"internalType": "uint256","name": "lltv","type": "uint256"}],"internalType": "struct MarketParams","name": "marketParams","type": "tuple"},{"internalType": "address","name": "borrower","type": "address"},{"internalType": "uint256","name": "seizedAssets","type": "uint256"},{"internalType": "uint256","name": "repaidShares","type": "uint256"},{"internalType": "bytes","name": "data","type": "bytes"}],"name": "liquidate","outputs": [{"internalType": "uint256","name": "","type": "uint256"},{"internalType": "uint256","name": "","type": "uint256"}],"stateMutability": "nonpayable","type": "function"},{"inputs": [{"internalType": "Id","name": "","type": "bytes32"}],"name": "market","outputs": [{"internalType": "uint128","name": "totalSupplyAssets","type": "uint128"},{"internalType": "uint128","name": "totalSupplyShares","type": "uint128"},{"internalType": "uint128","name": "totalBorrowAssets","type": "uint128"},{"internalType": "uint128","name": "totalBorrowShares","type": "uint128"},{"internalType": "uint128","name": "lastUpdate","type": "uint128"},{"internalType": "uint128","name": "fee","type": "uint128"}],"stateMutability": "view","type": "function"},{"inputs": [{"internalType": "address","name": "","type": "address"}],"name": "nonce","outputs": [{"internalType": "uint256","name": "","type": "uint256"}],"stateMutability": "view","type": "function"},{"inputs": [],"name": "owner","outputs": [{"internalType": "address","name": "","type": "address"}],"stateMutability": "view","type": "function"},{"inputs": [{"internalType": "Id","name": "","type": "bytes32"},{"internalType": "address","name": "","type": "address"}],"name": "position","outputs": [{"internalType": "uint256","name": "supplyShares","type": "uint256"},{"internalType": "uint128","name": "borrowShares","type": "uint128"},{"internalType": "uint128","name": "collateral","type": "uint128"}],"stateMutability": "view","type": "function"},{"inputs": [{"components": [{"internalType": "address","name": "loanToken","type": "address"},{"internalType": "address","name": "collateralToken","type": "address"},{"internalType": "address","name": "oracle","type": "address"},{"internalType": "address","name": "irm","type": "address"},{"internalType": "uint256","name": "lltv","type": "uint256"}],"internalType": "struct MarketParams","name": "marketParams","type": "tuple"},{"internalType": "uint256","name": "assets","type": "uint256"},{"internalType": "uint256","name": "shares","type": "uint256"},{"internalType": "address","name": "onBehalf","type": "address"},{"internalType": "bytes","name": "data","type": "bytes"}],"name": "repay","outputs": [{"internalType": "uint256","name": "","type": "uint256"},{"internalType": "uint256","name": "","type": "uint256"}],"stateMutability": "nonpayable","type": "function"},{"inputs": [{"internalType": "address","name": "authorized","type": "address"},{"internalType": "bool","name": "newIsAuthorized","type": "bool"}],"name": "setAuthorization","outputs": [],"stateMutability": "nonpayable","type": "function"},{"inputs": [{"components": [{"internalType": "address","name": "authorizer","type": "address"},{"internalType": "address","name": "authorized","type": "address"},{"internalType": "bool","name": "isAuthorized","type": "bool"},{"internalType": "uint256","name": "nonce","type": "uint256"},{"internalType": "uint256","name": "deadline","type": "uint256"}],"internalType": "struct Authorization","name": "authorization","type": "tuple"},{"components": [{"internalType": "uint8","name": "v","type": "uint8"},{"internalType": "bytes32","name": "r","type": "bytes32"},{"internalType": "bytes32","name": "s","type": "bytes32"}],"internalType": "struct Signature","name": "signature","type": "tuple"}],"name": "setAuthorizationWithSig","outputs": [],"stateMutability": "nonpayable","type": "function"},{"inputs": [{"components": [{"internalType": "address","name": "loanToken","type": "address"},{"internalType": "address","name": "collateralToken","type": "address"},{"internalType": "address","name": "oracle","type": "address"},{"internalType": "address","name": "irm","type": "address"},{"internalType": "uint256","name": "lltv","type": "uint256"}],"internalType": "struct MarketParams","name": "marketParams","type": "tuple"},{"internalType": "uint256","name": "newFee","type": "uint256"}],"name": "setFee","outputs": [],"stateMutability": "nonpayable","type": "function"},{"inputs": [{"internalType": "address","name": "newFeeRecipient","type": "address"}],"name": "setFeeRecipient","outputs": [],"stateMutability": "nonpayable","type": "function"},{"inputs": [{"internalType": "address","name": "newOwner","type": "address"}],"name": "setOwner","outputs": [],"stateMutability": "nonpayable","type": "function"},{"inputs": [{"components": [{"internalType": "address","name": "loanToken","type": "address"},{"internalType": "address","name": "collateralToken","type": "address"},{"internalType": "address","name": "oracle","type": "address"},{"internalType": "address","name": "irm","type": "address"},{"internalType": "uint256","name": "lltv","type": "uint256"}],"internalType": "struct MarketParams","name": "marketParams","type": "tuple"},{"internalType": "uint256","name": "assets","type": "uint256"},{"internalType": "uint256","name": "shares","type": "uint256"},{"internalType": "address","name": "onBehalf","type": "address"},{"internalType": "bytes","name": "data","type": "bytes"}],"name": "supply","outputs": [{"internalType": "uint256","name": "","type": "uint256"},{"internalType": "uint256","name": "","type": "uint256"}],"stateMutability": "nonpayable","type": "function"},{"inputs": [{"components": [{"internalType": "address","name": "loanToken","type": "address"},{"internalType": "address","name": "collateralToken","type": "address"},{"internalType": "address","name": "oracle","type": "address"},{"internalType": "address","name": "irm","type": "address"},{"internalType": "uint256","name": "lltv","type": "uint256"}],"internalType": "struct MarketParams","name": "marketParams","type": "tuple"},{"internalType": "uint256","name": "assets","type": "uint256"},{"internalType": "address","name": "onBehalf","type": "address"},{"internalType": "bytes","name": "data","type": "bytes"}],"name": "supplyCollateral","outputs": [],"stateMutability": "nonpayable","type": "function"},{"inputs": [{"components": [{"internalType": "address","name": "loanToken","type": "address"},{"internalType": "address","name": "collateralToken","type": "address"},{"internalType": "address","name": "oracle","type": "address"},{"internalType": "address","name": "irm","type": "address"},{"internalType": "uint256","name": "lltv","type": "uint256"}],"internalType": "struct MarketParams","name": "marketParams","type": "tuple"},{"internalType": "uint256","name": "assets","type": "uint256"},{"internalType": "uint256","name": "shares","type": "uint256"},{"internalType": "address","name": "onBehalf","type": "address"},{"internalType": "address","name": "receiver","type": "address"}],"name": "withdraw","outputs": [{"internalType": "uint256","name": "","type": "uint256"},{"internalType": "uint256","name": "","type": "uint256"}],"stateMutability": "nonpayable","type": "function"},{"inputs": [{"components": [{"internalType": "address","name": "loanToken","type": "address"},{"internalType": "address","name": "collateralToken","type": "address"},{"internalType": "address","name": "oracle","type": "address"},{"internalType": "address","name": "irm","type": "address"},{"internalType": "uint256","name": "lltv","type": "uint256"}],"internalType": "struct MarketParams","name": "marketParams","type": "tuple"},{"internalType": "uint256","name": "assets","type": "uint256"},{"internalType": "address","name": "onBehalf","type": "address"},{"internalType": "address","name": "receiver","type": "address"}],"name": "withdrawCollateral","outputs": [],"stateMutability": "nonpayable","type": "function"}]
            "#,
        )?;
        let provider = ethers::prelude::Provider::<ethers::providers::Http>::try_from(rpc)?;
        let client = Arc::new(provider);
        let token_address: ethers::types::Address = contract.parse()?;
        let contract = ethers::contract::Contract::new(token_address, abi, client.clone());
        let liquidity_adapter_address: ethers::types::Address =
            contract.method("liquidityAdapter", ())?.call().await?;
        let la_contract = ethers::contract::Contract::new(
            liquidity_adapter_address,
            la_abi.clone(),
            client.clone(),
        );

        let mut main_token_address: Option<ethers::types::Address> = None;
        if let Ok(method) = la_contract.method::<_, ethers::types::Address>("morpho", ()) {
            if let Ok(address) = method.call().await {
                main_token_address = Some(address);
            }
        }

        if main_token_address.is_none() {
            let adapters_length: U256 = contract.method("adaptersLength", ())?.call().await?;
            for adapter_index in 0..adapters_length.as_u64() {
                let adapter_address: ethers::types::Address = contract
                    .method("adapters", U256::from(adapter_index))?
                    .call()
                    .await?;
                let adapter_contract = ethers::contract::Contract::new(
                    adapter_address,
                    la_abi.clone(),
                    client.clone(),
                );
                if let Ok(method) =
                    adapter_contract.method::<_, ethers::types::Address>("morpho", ())
                {
                    if let Ok(address) = method.call().await {
                        main_token_address = Some(address);
                        break;
                    }
                }
            }
        }

        let main_token_address = main_token_address.ok_or("No Morpho adapter found")?;
        let main_contract =
            ethers::contract::Contract::new(main_token_address, main_abi, client.clone());
        let mut storage = metrics.lock().await;
        for addr in addresses.clone() {
            let key = format!("{}_balance_{}", name, addr);
            let value = super::base::Value::Int(prometheus::IntGauge::new(
                &key,
                format!("Value of {}", key),
            )?);
            storage.insert(key, value);
        }
        let key = format!(
            "avaliable_liquidity_{}",
            token_address.encode_hex_with_prefix()
        );
        storage.insert(
            format!("{}_{}", name, &key),
            super::base::Value::Int(prometheus::IntGauge::new(
                &format!("{}_{}", name, &key),
                format!(
                    "Free liquidity for {}",
                    token_address.encode_hex_with_prefix()
                ),
            )?),
        );
        let key = format!("idle_liquidity_{}", token_address.encode_hex_with_prefix());
        storage.insert(
            format!("{}_{}", name, &key),
            super::base::Value::Int(prometheus::IntGauge::new(
                &format!("{}_{}", name, &key),
                format!(
                    "Idle liquidity for {}",
                    token_address.encode_hex_with_prefix()
                ),
            )?),
        );

        Ok(Self {
            addresses: addresses
                .clone()
                .iter()
                .map(|addr| addr.parse().unwrap())
                .collect(),
            contract,
            token: token_address.encode_hex_with_prefix(),
            main_contract,
            la_contract,
            name: name.to_string(),
        })
    }

    async fn update_balances(
        &mut self,
        storage: &mut HashMap<String, super::base::Value>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for addr in self.addresses.clone() {
            let address: ethers::types::Address = addr.parse()?;
            let balance_shares: U256 = self
                .contract
                .method::<_, U256>("balanceOf", address)?
                .call()
                .await?;
            let balance: U256 = self
                .contract
                .method::<_, U256>("convertToAssets", balance_shares)?
                .call()
                .await?;
            let value = storage.get(&self.get_key(&format!("balance_{}", addr)));
            match value {
                Some(super::base::Value::Int(v)) => {
                    v.set(balance.as_u64() as i64);
                }
                _ => unreachable!(),
            }
        }
        Ok(())
    }

    async fn update_free_liquidity(
        &mut self,
        storage: &mut std::collections::HashMap<String, super::base::Value>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut free_liquidity = U256::zero();
        let mut idle_liquidity = U128::zero();

        let liquidity_data: Bytes = self
            .contract
            .method::<_, Bytes>("liquidityData", ())?
            .call()
            .await?;

        if let Some(params) = decode_liquidity_market_params(&liquidity_data) {
            let market_id = market_id(&params);

            let (
                total_supply_assets,
                total_supply_shares,
                total_borrow_assets,
                _total_borrow_shares,
                _last_update,
                _fee,
            ): (U128, U128, U128, U128, U128, U128) = self
                .main_contract
                .method::<_, (U128, U128, U128, U128, U128, U128)>("market", market_id)?
                .call()
                .await?;

            let (supply_shares, _borrow_shares, _collateral_assets): (U256, U128, U128) = self
                .main_contract
                .method::<_, (U256, U128, U128)>(
                    "position",
                    (market_id, self.la_contract.address()),
                )?
                .call()
                .await?;

            let vault_supply_assets = mul_div_floor(
                supply_shares,
                total_supply_assets.into(),
                total_supply_shares.into(),
            );
            let market_available = total_supply_assets
                .checked_sub(total_borrow_assets)
                .unwrap_or_default();
            let vault_available = vault_supply_assets.min(market_available.into());

            free_liquidity = vault_available;
        } else {
            // No MorphoMarketV1AdapterV2 liquidityData. If the liquidity adapter is not a Morpho
            // market-list adapter, do not use `realAssets()` here: that is total deposits allocated
            // through the adapter, not withdrawable liquidity.
            let mut market_ids_length = None;
            if let Ok(method) = self.la_contract.method::<_, U256>("marketIdsLength", ()) {
                if let Ok(length) = method.call().await {
                    market_ids_length = Some(length);
                }
            }

            let market_ids_length = match market_ids_length {
                Some(length) => length,
                None => {
                    // MorphoVaultV1Adapter: liquidity is the amount the adapter can withdraw from
                    // the underlying MetaMorpho/V1 vault, not the adapter's `realAssets()` total position.
                    let morpho_vault_v1_adapter_abi: Abi = serde_json::from_str(
                        r#"[{"inputs":[],"name":"morphoVaultV1","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"}]"#,
                    )?;
                    let morpho_vault_v1_adapter = ethers::contract::Contract::new(
                        self.la_contract.address(),
                        morpho_vault_v1_adapter_abi,
                        self.contract.client(),
                    );
                    let morpho_vault_v1 = morpho_vault_v1_adapter
                        .method::<_, Address>("morphoVaultV1", ())?
                        .call()
                        .await?;
                    let morpho_vault_v1_abi: Abi = serde_json::from_str(
                        r#"[{"inputs":[{"internalType":"address","name":"owner","type":"address"}],"name":"maxWithdraw","outputs":[{"internalType":"uint256","name":"maxAssets","type":"uint256"}],"stateMutability":"view","type":"function"}]"#,
                    )?;
                    let morpho_vault_v1_contract = ethers::contract::Contract::new(
                        morpho_vault_v1,
                        morpho_vault_v1_abi,
                        self.contract.client(),
                    );
                    free_liquidity = morpho_vault_v1_contract
                        .method::<_, U256>("maxWithdraw", self.la_contract.address())?
                        .call()
                        .await?;
                    U256::zero()
                }
            };

            for queue_index in 0..market_ids_length.as_u64() {
                let market_id: [u8; 32] = self
                    .la_contract
                    .method::<_, [u8; 32]>("marketIds", U256::from(queue_index))?
                    .call()
                    .await?;

                let (
                    total_supply_assets,
                    total_supply_shares,
                    total_borrow_assets,
                    _total_borrow_shares,
                    _last_update,
                    _fee,
                ): (U128, U128, U128, U128, U128, U128) = self
                    .main_contract
                    .method::<_, (U128, U128, U128, U128, U128, U128)>("market", market_id)?
                    .call()
                    .await?;

                let (_loan_token, collateral_token, oracle, irm, lltv): (
                    Address,
                    Address,
                    Address,
                    Address,
                    U256,
                ) = self
                    .main_contract
                    .method::<_, (Address, Address, Address, Address, U256)>(
                        "idToMarketParams",
                        market_id,
                    )?
                    .call()
                    .await?;

                let (supply_shares, _borrow_shares, _collateral_assets): (U256, U128, U128) = self
                    .main_contract
                    .method::<_, (U256, U128, U128)>(
                        "position",
                        (market_id, self.la_contract.address()),
                    )?
                    .call()
                    .await?;

                if lltv.is_zero() && collateral_token.is_zero() && oracle.is_zero() && irm.is_zero()
                {
                    idle_liquidity = idle_liquidity.saturating_add(U128::from(
                        mul_div_floor(
                            supply_shares.into(),
                            total_supply_assets.into(),
                            total_supply_shares.into(),
                        )
                        .as_u128(),
                    ));
                    continue;
                }

                let vault_supply_assets = mul_div_floor(
                    supply_shares,
                    total_supply_assets.into(),
                    total_supply_shares.into(),
                );

                let market_available = total_supply_assets
                    .checked_sub(total_borrow_assets)
                    .unwrap_or_default();

                let vault_available = vault_supply_assets.min(market_available.into());

                free_liquidity = free_liquidity.saturating_add(vault_available);
            }
        }

        let key = format!("idle_liquidity_{}", self.token);
        match storage.get(&self.get_key(&key)) {
            Some(super::base::Value::Int(v)) => {
                v.set(idle_liquidity.as_u64() as i64);
            }
            _ => unreachable!(),
        }

        let key = format!("avaliable_liquidity_{}", self.token);
        match storage.get(&self.get_key(&key)) {
            Some(super::base::Value::Int(v)) => {
                v.set(free_liquidity.as_u64() as i64);
            }
            _ => unreachable!(),
        }

        Ok(())
    }
}
