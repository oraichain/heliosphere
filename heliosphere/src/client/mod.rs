use std::{
    collections::{BTreeMap, HashMap},
    time::{Duration, Instant},
};

use heliosphere_core::{
    block::{Block, BlockBy, BlockHeader},
    event::{EventData, EventsResult},
    transaction::{Transaction, TransactionId},
    util::extract_sig_from_event,
    Address,
};
// use reqwest::{Client, IntoUrl, Url};
use awc::http::header;
use awc::Client;
use serde::{de::DeserializeOwned, Serialize};

use self::types::{
    AccountBalanceResponse, BroadcastTxResponse, ChainParametersResponse, QueryContractResponse,
    TransactionInfo, TriggerContractResponse,
};

mod types;
pub use types::{AccountResources, ResourceType};

/// Method call params
pub struct MethodCall<'a> {
    /// Issuer of contract call, msg.sender
    pub caller: &'a Address,
    /// Contract address
    pub contract: &'a Address,
    /// Method signature string e.g. `transfer(address,uint256)`
    pub selector: &'a str,
    /// ABI encoded arguments (e.g. with ethabi crate)
    pub parameter: &'a [u8],
}

/// Builder struct for RpcClient
pub struct RpcClientBuilder {
    client: Option<Client>,
    poll_interval: Duration,
    rpc_url: String,
    timeout: Duration,
}

impl RpcClientBuilder {
    /// Create new instance
    pub fn new<U>(rpc_url: U, timeout: Duration) -> Result<Self, crate::Error>
    where
        U: ToString,
    {
        Ok(Self {
            client: None,
            timeout,
            poll_interval: Duration::from_secs(5),
            rpc_url: rpc_url.to_string(),
        })
    }

    /// Set custom reqwest::Client instance
    pub fn with_client(mut self, client: Client) -> Self {
        self.client = Some(client);
        self
    }

    /// Set custom tx confirmation poll interval (default 5 seconds)
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Build new RpcClient instance
    pub fn build(self) -> RpcClient {
        RpcClient {
            rpc_url: self.rpc_url,
            client: self.client.unwrap_or_default(),
            poll_interval: self.poll_interval,
            timeout: self.timeout,
            headers: HashMap::new(),
        }
    }
}

/// RpcClient for creating and broadcasting transaction or interaction with smart contracts
#[derive(Clone)]
pub struct RpcClient {
    rpc_url: String,
    client: Client,
    poll_interval: Duration,
    timeout: Duration,
    headers: HashMap<String, String>,
}

impl RpcClient {
    /// Create new RpcClient with default params
    pub fn new<U>(rpc_url: U, timeout: Duration) -> Result<Self, crate::Error>
    where
        U: ToString,
    {
        Ok(RpcClientBuilder::new(rpc_url, timeout)?.build())
    }

    /// return timeout
    pub fn get_timeout(&self) -> Duration {
        self.timeout
    }

    /// set a custom header
    pub fn set_header(&mut self, key: &str, value: &str) {
        self.headers.insert(key.to_string(), value.to_string());
    }

    /// return header
    pub fn get_header(&self, key: &str) -> String {
        self.headers.get(key).cloned().unwrap_or_default()
    }

    /// return all header keys
    pub fn header_keys(&self) -> Vec<String> {
        self.headers.keys().map(|k| k.clone()).collect()
    }

    /// Send a POST request with json-serializable payload
    pub async fn api_post<P, R>(&self, method: &str, payload: &P) -> Result<R, crate::Error>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        let mut request = self
            .client
            .post(&format!("{}{}", self.rpc_url, method))
            .timeout(self.timeout)
            .append_header((header::CONTENT_TYPE, "application/json"));

        for (key, value) in &self.headers {
            request = request.insert_header((key.clone(), value.clone()));
        }

        Ok(request.send_json(payload).await?.json().await?)
    }

    /// Send a GET request
    pub async fn api_get<R>(&self, method: &str) -> Result<R, crate::Error>
    where
        R: DeserializeOwned,
    {
        let mut request = self
            .client
            .get(&format!("{}{}", self.rpc_url, method))
            .timeout(self.timeout)
            .append_header((header::CONTENT_TYPE, "application/json"));

        for (key, value) in &self.headers {
            request = request.insert_header((key.clone(), value.clone()));
        }

        Ok(request.send().await?.json().await?)
    }

    /// Broadcast signed transaction
    pub async fn broadcast_transaction(
        &self,
        tx: &Transaction,
    ) -> Result<TransactionId, crate::Error> {
        let resp: BroadcastTxResponse = self.api_post("/wallet/broadcasttransaction", tx).await?;
        match resp.code {
            Some(err) => Err(crate::Error::TxConstructionFailed(err, resp.message)),
            None => Ok(resp.txid),
        }
    }

    /// Get latest block
    pub async fn get_latest_block(&self) -> Result<Block, crate::Error> {
        self.api_post("/wallet/getnowblock", &serde_json::json!({}))
            .await
    }

    /// Get block by id or number
    pub async fn get_block(&self, by: BlockBy) -> Result<Block, crate::Error> {
        self.api_post(
            "/wallet/getblock",
            &serde_json::json!({
                "id_or_num": by.id_or_num(),
                "detail": true,
            }),
        )
        .await
    }

    /// Get only block header
    pub async fn get_block_header(&self, by: BlockBy) -> Result<BlockHeader, crate::Error> {
        let block: Block = self
            .api_post(
                "/wallet/getblock",
                &serde_json::json!({
                    "id_or_num": by.id_or_num(),
                    "detail": false,
                }),
            )
            .await?;
        Ok(block.block_header)
    }

    /// Get transaction info
    pub async fn get_tx_info_by_id(
        &self,
        txid: TransactionId,
    ) -> Result<Option<TransactionInfo>, crate::Error> {
        let res: serde_json::Value = self
            .api_post(
                "/walletsolidity/gettransactionbyid",
                &serde_json::json!({ "value": txid }),
            )
            .await?;
        if res.get("txID").is_none() {
            return Ok(None);
        } // does not exist or unconfirmed
        serde_json::from_value(res).map_err(|e| crate::Error::UnknownResponse(e.to_string()))
    }

    /// Await transaction confirmation
    pub async fn await_confirmation(
        &self,
        txid: TransactionId,
        timeout: Duration,
    ) -> Result<TransactionInfo, crate::Error> {
        let start = Instant::now();
        loop {
            let info = self.get_tx_info_by_id(txid).await?;
            match info {
                Some(x) if !x.ret.is_empty() && x.ret[0].contract_ret == "SUCCESS" => return Ok(x),
                Some(x) => {
                    return Err(crate::Error::TxFailed(
                        x.ret
                            .get(0)
                            .map(|x| x.contract_ret.clone())
                            .unwrap_or_else(|| "empty ret".to_owned()),
                    ))
                }
                _ => {
                    tokio::time::sleep(self.poll_interval).await;
                }
            }
            if Instant::now() - start > timeout {
                return Err(crate::Error::TxTimeout);
            }
        }
    }

    /** Create a TRX transfer transaction
     ** from - Sender address
     ** to - Receiver address
     ** amount - Raw amount of TRX to transfer in SUN (1 TRX = 1,000,000 SUN)
     */
    pub async fn trx_transfer(
        &self,
        from: &Address,
        to: &Address,
        amount: u64,
    ) -> Result<Transaction, crate::Error> {
        self.api_post(
            "/wallet/createtransaction",
            &serde_json::json!({
                "owner_address": from.as_hex(),
                "to_address": to.as_hex(),
                "amount": amount,
                "extra_data": hex::encode([0x72; 64]),
            }),
        )
        .await
    }

    /** Create an account
     ** payer - Activated account from which account creation fee should be deduced
     ** account - Account address to create (must be calculated in advance e.g. from existing private key)
     */
    pub async fn create_account(
        &self,
        payer: &Address,
        account: &Address,
    ) -> Result<Transaction, crate::Error> {
        self.api_post(
            "/wallet/createaccount",
            &serde_json::json!({
                "owner_address": payer.as_hex(),
                "account_address": account.as_hex(),
            }),
        )
        .await
    }

    /** Stake an amount of TRX to obtain bandwidth OR Energy and TRON Power (voting rights).
     ** owner - Source of staked TRX
     ** amount - Amount of TRX to stake in SUN (1 TRX = 1,000,000 SUN)
     ** resource - Stake for Energy or Bandwidth
     ** receiver_address - Optional, can be used to delegate obtained energy or bandwidth to another address
     */
    pub async fn freeze_balance(
        &self,
        owner: &Address,
        amount: u64,
        resource: ResourceType,
        receiver_address: Option<&Address>,
    ) -> Result<Transaction, crate::Error> {
        self.api_post(
            "/wallet/freezebalance",
            &serde_json::json!({
                "owner_address": owner.as_hex(),
                "frozen_balance": amount,
                "frozen_duration": 3_u8,
                "resource": resource,
                "receiver_address": receiver_address.map(|x| x.as_hex()),
            }),
        )
        .await
    }

    /** Unstake TRX
     ** owner - Source of staked TRX
     ** resource - Stake for Energy or Bandwidth
     ** receiver_address - Optional, if resources were delegated to another address
     */
    pub async fn unfreeze_balance(
        &self,
        owner: &Address,
        resource: ResourceType,
        receiver_address: Option<&Address>,
    ) -> Result<Transaction, crate::Error> {
        self.api_post(
            "/wallet/unfreezebalance",
            &serde_json::json!({
                "owner_address": owner.as_hex(),
                "resource": resource,
                "receiver_address": receiver_address.map(|x| x.as_hex()),
            }),
        )
        .await
    }

    /** Call a smart contract method
     ** method_call: Call parameters
     ** value - Amount of TRX in SUN to send along with method call
     ** fee_limit - Maximum TRX consumption, measured in SUN (1 TRX = 1,000,000 SUN)
     */
    pub async fn trigger_contract(
        &self,
        method_call: &MethodCall<'_>,
        value: u64,
        fee_limit: Option<u64>,
    ) -> Result<Transaction, crate::Error> {
        let fee_limit = match fee_limit {
            Some(fee_limit) => fee_limit,
            None => self.estimate_fee_limit(method_call).await?,
        };
        let resp: TriggerContractResponse = self
            .api_post(
                "/wallet/triggersmartcontract",
                &serde_json::json!({
                    "owner_address": method_call.caller.as_hex(),
                    "contract_address": method_call.contract.as_hex(),
                    "function_selector": method_call.selector,
                    "parameter": hex::encode(method_call.parameter),
                    "fee_limit": fee_limit,
                    "call_value": value
                }),
            )
            .await?;
        Ok(resp.transaction)
    }

    /** Query a smart contract view method
     ** method_call: Call parameters
     */
    pub async fn query_contract(
        &self,
        method_call: &MethodCall<'_>,
    ) -> Result<QueryContractResponse, crate::Error> {
        let resp: QueryContractResponse = self
            .api_post(
                "/wallet/triggerconstantcontract",
                &serde_json::json!({
                    "owner_address": method_call.caller.as_hex(),
                    "contract_address": method_call.contract.as_hex(),
                    "function_selector": method_call.selector,
                    "parameter": hex::encode(method_call.parameter),
                }),
            )
            .await?;
        if resp.constant_result.is_empty() && resp.code.is_none() {
            return Err(crate::Error::ContractNotFound);
        }

        if let Some(code) = resp.code.as_ref() {
            return Err(crate::Error::ContractQueryFailed(
                code.to_owned(),
                resp.message,
            ));
        }
        Ok(resp)
    }

    /** Estimate energy cost of given smart contract call
     ** method_call: Call parameters
     */
    pub async fn estimate_energy(&self, method_call: &MethodCall<'_>) -> Result<u64, crate::Error> {
        let resp = self.query_contract(method_call).await?;
        Ok(resp.energy_used)
    }

    /** Estimate fee limit of given smart contract call
     ** method_call: Call parameters
     */
    pub async fn estimate_fee_limit(
        &self,
        method_call: &MethodCall<'_>,
    ) -> Result<u64, crate::Error> {
        let params = self.get_chain_parameters().await?;
        let energy_fee = *params
            .get("getEnergyFee")
            .ok_or_else(|| crate::Error::UnknownResponse("getEnergyFee not found".to_owned()))?
            as u64;
        Ok(self.estimate_energy(method_call).await? * energy_fee)
    }

    /// Query the resource information of an account (bandwidth, energy, etc..)
    pub async fn get_account_resources(
        &self,
        account: &Address,
    ) -> Result<AccountResources, crate::Error> {
        self.api_post(
            "/wallet/getaccountresource",
            &serde_json::json!({"address": account.as_hex()}),
        )
        .await
    }

    /// Query TRX account balance (including frozen)
    pub async fn get_account_balance(&self, account: &Address) -> Result<u64, crate::Error> {
        let resp: AccountBalanceResponse = self
            .api_post(
                "/wallet/getaccount",
                &serde_json::json!({ "address": account.as_hex() }),
            )
            .await?;

        resp.balance.ok_or(crate::Error::AccountNotFound)
    }

    /// All parameters that the blockchain committee can set
    pub async fn get_chain_parameters(&self) -> Result<BTreeMap<String, i64>, crate::Error> {
        let resp: ChainParametersResponse = self.api_get("/wallet/getchainparameters").await?;
        Ok(resp
            .chain_parameter
            .into_iter()
            .filter_map(|p| Some((p.key, p.value?)))
            .collect())
    }

    /// Return all matched topics events
    pub async fn check_for_events(
        &self,
        start_block: u64,
        end_block: Option<u64>,
        contract_address: Address,
        selector: &str,
    ) -> Result<Vec<EventData>, crate::Error> {
        let method_name = &selector[..selector.find('(').unwrap_or(selector.len())];

        let block_header = self.get_block_header(BlockBy::Number(start_block)).await?;

        let mut url = format!(
            "/v1/contracts/{}/events?event_name={}&only_confirmed=true&order_by=block_timestamp,asc&limit=200&min_block_timestamp={}",
            contract_address.as_base58(),
            method_name,
            block_header.raw_data.timestamp
        );

        if let Some(end_block) = end_block {
            let block_header = self.get_block_header(BlockBy::Number(end_block)).await?;
            url = format!(
                "{}&max_block_timestamp={}",
                url, block_header.raw_data.timestamp
            );
        }

        let res: EventsResult = self.api_get(&url).await?;

        if !res.success {
            return Err(crate::Error::ApiError(
                res.error.unwrap_or("Api Error".to_string()),
            ));
        }

        Ok(res
            .data
            .into_iter()
            .filter(|e| extract_sig_from_event(&e.event).eq(selector))
            .collect())
    }
}
