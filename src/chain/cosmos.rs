use std::{
    cmp::min,
    future::Future,
    str::FromStr,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use bech32::{ToBase32, Variant};
use bitcoin::hashes::hex::ToHex;
use itertools::Itertools;
use prost::Message;
use prost_types::Any;
use tendermint::account::Id as AccountId;
use tendermint_rpc::query::{Query,EventType};
use tendermint_rpc::endpoint::tx::Response as ResultTx;
use tendermint_rpc::{endpoint::broadcast::tx_sync::Response, Client, HttpClient, Order};
use tokio::runtime::Runtime as TokioRuntime;
use tonic::codegen::http::Uri;
use tracing::{debug, trace, warn, info};
use crate::cosmos::auth::v1beta1::{BaseAccount, QueryAccountRequest};
use crate::cosmos::base::tendermint::v1beta1::service_client::ServiceClient;
use crate::cosmos::base::tendermint::v1beta1::GetNodeInfoRequest;
use crate::cosmos::base::v1beta1::Coin;
use crate::cosmos::tx::v1beta1::mode_info::{Single, Sum};
use crate::cosmos::tx::v1beta1::{
    AuthInfo, Fee, ModeInfo, SignDoc, SignerInfo, SimulateRequest, SimulateResponse, Tx, TxBody,
    TxRaw,
};
use crate::events::Event;
use crate::error::{Error, Kind};
use crate::subscribe::monitor::{EventMonitor, EventReceiver};
use crate::keyring::{KeyEntry, KeyRing, Store};
use crate::{subscribe::monitor::TxMonitorCmd};
use rust_decimal::prelude::*;
use super::Chain;
use tendermint::chain::Id as ChainId;
use crate::chain::{QueryTxRequest, QueryTxHash};
use crate::config::{ChainConfig,GasPrice};

const DEFAULT_MAX_GAS: u64 = 3_000_000;
const DEFAULT_GAS_PRICE_ADJUSTMENT: f64 = 0.1;

const DEFAULT_MAX_MSG_NUM: usize = 30;
const DEFAULT_MAX_TX_SIZE: usize = 2 * 1048576; // 2 MBytes

mod retry_strategy {
    use crate::util::retry::Fixed;
    use std::time::Duration;

    pub fn wait_for_block_commits(max_total_wait: Duration) -> impl Iterator<Item = Duration> {
        let backoff_millis = 300; // The periodic backoff
        let count: usize = (max_total_wait.as_millis() / backoff_millis as u128) as usize;
        Fixed::from_millis(backoff_millis).take(count)
    }
}

pub struct CosmosSdkChain {
    config: ChainConfig,
    rpc_client: HttpClient,
    grpc_addr: Uri,
    rt: Arc<TokioRuntime>,
    keybase: KeyRing,
    /// A cached copy of the account information
    account: Option<BaseAccount>,
}

impl CosmosSdkChain {
    /// Does multiple RPC calls to the full node, to check for
    /// reachability and that some basic APIs are available.
    ///
    /// Currently this checks that:
    ///     - the node responds OK to `/health` RPC call;
    ///     - the node has transaction indexing enabled;
    ///     - the SDK version is supported.
    ///
    /// Emits a log warning in case anything is amiss.
    /// Exits early if any health check fails, without doing any
    /// further checks.
    fn health_checkup(&self) {
        async fn do_health_checkup(chain: &CosmosSdkChain) -> Result<(), Error> {
            let chain_id = chain.id();
            let grpc_address = chain.grpc_addr.to_string();
            let rpc_address = chain.config.rpc_addr.to_string();

            // Checkup on the self-reported health endpoint
            chain
                .rpc_client
                .health()
                .await
                .map_err(|e| Kind::HealthCheckJsonRpc {
                    chain_id: chain_id.clone(),
                    address: rpc_address.clone(),
                    endpoint: "/health".to_string(),
                    cause: e,
                })?;

            // Checkup on transaction indexing
            chain
                .rpc_client
                .tx_search(
                    Query::from(EventType::NewBlock),
                    false,
                    1,
                    1,
                    Order::Ascending,
                )
                .await
                .map_err(|e| Kind::HealthCheckJsonRpc {
                    chain_id: chain_id.clone(),
                    address: rpc_address.clone(),
                    endpoint: "/tx_search".to_string(),
                    cause: e,
                })?;

            let mut client = ServiceClient::connect(chain.grpc_addr.clone())
                .await
                .map_err(|e| {
                    // Failed to create the gRPC client to call into `/node_info`.
                    Kind::HealthCheckGrpc {
                        chain_id: chain_id.clone(),
                        address: grpc_address.clone(),
                        endpoint: "tendermint::ServiceClient".to_string(),
                        cause: e.to_string(),
                    }
                })?;

            let request = tonic::Request::new(GetNodeInfoRequest {});

            let response =
                client
                    .get_node_info(request)
                    .await
                    .map_err(|e| Kind::HealthCheckGrpc {
                        chain_id: chain_id.clone(),
                        address: grpc_address.clone(),
                        endpoint: "tendermint::GetNodeInfoRequest".to_string(),
                        cause: e.to_string(),
                    })?;

            let version =
                response
                    .into_inner()
                    .application_version
                    .ok_or_else(|| Kind::HealthCheckGrpc {
                        chain_id: chain_id.clone(),
                        address: grpc_address.clone(),
                        endpoint: "tendermint::GetNodeInfoRequest".to_string(),
                        cause: "the gRPC response contains no application version information"
                            .to_string(),
                    })?;
            info!("version: {:?}", version);
            Ok(())
        }

        if let Err(e) = self.block_on(do_health_checkup(self)) {
            warn!("{}", e);
            warn!("some Hermes features may not work in this mode!");
        }
    }

    fn rpc_client(&self) -> &HttpClient {
        &self.rpc_client
    }

    pub fn config(&self) -> &ChainConfig {
        &self.config
    }

    /// Run a future to completion on the Tokio runtime.
    fn block_on<F: Future>(&self, f: F) -> F::Output {
        crate::time!("block_on");
        self.rt.block_on(f)
    }

    fn send_tx(&mut self, proto_msgs: Vec<Any>) -> Result<Response, Error> {
        crate::time!("send_tx");
        // forced refresh account when send tx
        self.account = None;
        let account_seq = self.account_sequence()?;

        debug!(
            "[{}] send_tx: sending {} messages using nonce {}",
            self.id(),
            proto_msgs.len(),
            account_seq,
        );

        let signer_info = self.signer(account_seq)?;
        let fee = self.default_fee();
        let (body, body_buf) = tx_body_and_bytes(proto_msgs)?;

        let (auth_info, auth_buf) = auth_info_and_bytes(signer_info.clone(), fee.clone())?;
        let signed_doc = self.signed_doc(body_buf.clone(), auth_buf, account_seq)?;

        // Try to simulate the Tx.
        // It is possible that a batch of messages are fragmented by the caller (`send_msgs`) such that
        // they do not individually verify. For example for the following batch
        // [`MsgUpdateClient`, `MsgRecvPacket`, ..., `MsgRecvPacket`]
        // if the batch is split in two TX-es, the second one will fail the simulation in `deliverTx` check
        // In this case we just leave the gas un-adjusted, i.e. use `self.max_gas()`
        let estimated_gas = self
            .send_tx_simulate(SimulateRequest {
                tx: Some(Tx {
                    body: Some(body),
                    auth_info: Some(auth_info),
                    signatures: vec![signed_doc],
                }),
            })
            .map_or(self.max_gas(), |sr| {
                sr.gas_info.map_or(self.max_gas(), |g| g.gas_used)
            });

        if estimated_gas > self.max_gas() {
            return Err(Kind::TxSimulateGasEstimateExceeded {
                chain_id: self.id().clone(),
                estimated_gas,
                max_gas: self.max_gas(),
            }
            .into());
        }

        let adjusted_fee = self.fee_with_gas(estimated_gas);

        trace!(
            "[{}] send_tx: based on the estimated gas, adjusting fee from {:?} to {:?}",
            self.id(),
            fee,
            adjusted_fee
        );

        let (_auth_adjusted, auth_buf_adjusted) = auth_info_and_bytes(signer_info, adjusted_fee)?;
        let account_number = self.account_number()?;
        let signed_doc =
            self.signed_doc(body_buf.clone(), auth_buf_adjusted.clone(), account_number)?;

        let tx_raw = TxRaw {
            body_bytes: body_buf,
            auth_info_bytes: auth_buf_adjusted,
            signatures: vec![signed_doc],
        };

        let mut tx_bytes = Vec::new();
        prost::Message::encode(&tx_raw, &mut tx_bytes).unwrap();

        let response = self
            .block_on(broadcast_tx_sync(self, tx_bytes))
            .map_err(|e| Kind::Rpc(self.config.rpc_addr.clone()).context(e))?;

        debug!("[{}] send_tx: broadcast_tx_sync: {:?}", self.id(), response);

        self.incr_account_sequence()?;

        Ok(response)
    }

    /// The maximum amount of gas the relayer is willing to pay for a transaction
    fn max_gas(&self) -> u64 {
        self.config.max_gas.unwrap_or(DEFAULT_MAX_GAS)
    }

    /// The gas price
    fn gas_price(&self) -> &GasPrice {
        &self.config.gas_price
    }

    /// The gas price adjustment
    fn gas_adjustment(&self) -> f64 {
        self.config
            .gas_adjustment
            .unwrap_or(DEFAULT_GAS_PRICE_ADJUSTMENT)
    }

    /// Adjusts the fee based on the configured `gas_adjustment` to prevent out of gas errors.
    /// The actual gas cost, when a transaction is executed, may be slightly higher than the
    /// one returned by the simulation.
    fn apply_adjustment_to_gas(&self, gas_amount: u64) -> u64 {
        let adjustment = (Decimal::from_u64(gas_amount).unwrap() * Decimal::from_f64(self.gas_adjustment()).unwrap()).ceil().to_u64().unwrap();
        min(
            gas_amount + adjustment,
            self.max_gas()
        )
    }


    /// The maximum fee the relayer pays for a transaction
    fn max_fee_in_coins(&self) -> Coin {
        calculate_fee(self.max_gas(), self.gas_price())
    }

    /// The fee in coins based on gas amount
    fn fee_from_gas_in_coins(&self, gas: u64) -> Coin {
        calculate_fee(gas, self.gas_price())
    }

    /// The maximum number of messages included in a transaction
    fn max_msg_num(&self) -> usize {
        self.config.max_msg_num.unwrap_or(DEFAULT_MAX_MSG_NUM)
    }

    /// The maximum size of any transaction sent by the relayer to this chain
    fn max_tx_size(&self) -> usize {
        self.config.max_tx_size.unwrap_or(DEFAULT_MAX_TX_SIZE)
    }

    fn send_tx_simulate(&self, request: SimulateRequest) -> Result<SimulateResponse, Error> {
        crate::time!("tx simulate");

        let mut client = self
            .block_on(
                crate::cosmos::tx::v1beta1::service_client::ServiceClient::connect(
                    self.grpc_addr.clone(),
                ),
            )
            .map_err(|e| Kind::Grpc.context(e))?;

        let request = tonic::Request::new(request);
        let response = self
            .block_on(client.simulate(request))
            .map_err(|e| Kind::Grpc.context(e))?
            .into_inner();

        Ok(response)
    }

    fn key(&self) -> Result<KeyEntry, Error> {
        Ok(self
            .keybase()
            .get_key(&self.config.key_name)
            .map_err(|e| Kind::KeyBase.context(e))?)
    }

    fn key_bytes(&self, key: &KeyEntry) -> Result<Vec<u8>, Error> {
        let mut pk_buf = Vec::new();
        prost::Message::encode(&key.public_key.public_key.to_bytes(), &mut pk_buf).unwrap();
        Ok(pk_buf)
    }

    fn key_and_bytes(&self) -> Result<(KeyEntry, Vec<u8>), Error> {
        let key = self.key()?;
        let key_bytes = self.key_bytes(&key)?;
        Ok((key, key_bytes))
    }

    fn account(&mut self) -> Result<&mut BaseAccount, Error> {
        if self.account == None {
            let account = self
                .block_on(query_account(self, self.key()?.account))
                .map_err(|e| Kind::Grpc.context(e))?;

            debug!(
                sequence = %account.sequence,
                number = %account.account_number,
                "[{}] send_tx: retrieved account",
                self.id()
            );

            self.account = Some(account);
        }

        Ok(self
            .account
            .as_mut()
            .expect("account was supposedly just cached"))
    }

    fn account_number(&mut self) -> Result<u64, Error> {
        Ok(self.account()?.account_number)
    }

    fn account_sequence(&mut self) -> Result<u64, Error> {
        Ok(self.account()?.sequence)
    }

    fn incr_account_sequence(&mut self) -> Result<(), Error> {
        self.account()?.sequence += 1;
        Ok(())
    }

    fn signer(&self, sequence: u64) -> Result<SignerInfo, Error> {
        let (_key, pk_buf) = self.key_and_bytes()?;
        // Create a MsgSend proto Any message
        let pk_any = Any {
            type_url: "/cosmos.crypto.secp256k1.PubKey".to_string(),
            value: pk_buf,
        };

        let single = Single { mode: 1 };
        let sum_single = Some(Sum::Single(single));
        let mode = Some(ModeInfo { sum: sum_single });
        let signer_info = SignerInfo {
            public_key: Some(pk_any),
            mode_info: mode,
            sequence,
        };
        Ok(signer_info)
    }

    fn default_fee(&self) -> Fee {
        Fee {
            amount: vec![self.max_fee_in_coins()],
            gas_limit: self.max_gas(),
            payer: "".to_string(),
            granter: "".to_string(),
        }
    }

    fn fee_with_gas(&self, gas_limit: u64) -> Fee {
        let adjusted_gas_limit = self.apply_adjustment_to_gas(gas_limit);
        Fee {
            amount: vec![self.fee_from_gas_in_coins(adjusted_gas_limit)],
            gas_limit: adjusted_gas_limit,
            ..self.default_fee()
        }
    }

    fn signed_doc(
        &self,
        body_bytes: Vec<u8>,
        auth_info_bytes: Vec<u8>,
        account_number: u64,
    ) -> Result<Vec<u8>, Error> {
        let sign_doc = SignDoc {
            body_bytes,
            auth_info_bytes,
            chain_id: self.config.clone().id.to_string(),
            account_number,
        };

        // A protobuf serialization of a SignDoc
        let mut signdoc_buf = Vec::new();
        prost::Message::encode(&sign_doc, &mut signdoc_buf).unwrap();

        // Sign doc
        let signed = self
            .keybase
            .sign_msg(&self.config.key_name, signdoc_buf)
            .map_err(|e| Kind::KeyBase.context(e))?;

        Ok(signed)
    }

    /// Given a vector of `TxSyncResult` elements,
    /// each including a transaction response hash for one or more messages, periodically queries the chain
    /// with the transaction hashes to get the list of IbcEvents included in those transactions.
    pub fn wait_for_block_commits(
        &self,
        mut tx_sync_results: Vec<TxSyncResult>,
    ) -> Result<Vec<TxSyncResult>, Error> {
        use crate::util::retry::{retry_with_index, RetryResult};

        let hashes = tx_sync_results
            .iter()
            .map(|res| res.response.hash.to_string())
            .join(", ");

        debug!("[{}] waiting for commit of block(s) {}", self.id(), hashes);

        // Wait a little bit initially
        thread::sleep(Duration::from_millis(200));

        let start = Instant::now();
        let result = retry_with_index(
            retry_strategy::wait_for_block_commits(self.config.rpc_timeout),
            |index| {
                if all_tx_results_found(&tx_sync_results) {
                    trace!(
                        "[{}] wait_for_block_commits: retrieved {} tx results after {} tries ({}ms)",
                        self.id(),
                        tx_sync_results.len(),
                        index,
                        start.elapsed().as_millis()
                    );

                    // All transactions confirmed
                    return RetryResult::Ok(());
                }

                for TxSyncResult { response, events } in tx_sync_results.iter_mut() {
                    // If this transaction was not committed, determine whether it was because it failed
                    // or because it hasn't been committed yet.
                    if empty_event_present(&events) {
                        // If the transaction failed, replace the events with an error,
                        // so that we don't attempt to resolve the transaction later on.
                        if response.code.value() != 0 {
                            *events = vec![Event::ChainError(format!(
                            "deliver_tx on chain {} for Tx hash {} reports error: code={:?}, log={:?}",
                            self.id(), response.hash, response.code, response.log
                        ))];

                        // Otherwise, try to resolve transaction hash to the corresponding events.
                        } else if let Ok(events_per_tx) =
                            self.query_events_from_txs(QueryTxRequest::Transaction(QueryTxHash(response.hash)))
                        {
                            // If we get events back, progress was made, so we replace the events
                            // with the new ones. in both cases we will check in the next iteration
                            // whether or not the transaction was fully committed.
                            if !events_per_tx.is_empty() {
                                *events = events_per_tx;
                            }
                        }
                    }
                }
                RetryResult::Retry(index)
            },
        );

        match result {
            // All transactions confirmed
            Ok(()) => Ok(tx_sync_results),
            // Did not find confirmation
            Err(_) => Err(Kind::TxNoConfirmation(format!(
                "from chain {} for hash(es) {}",
                self.id(),
                hashes
            ))
            .into()),
        }
    }
}

fn empty_event_present(events: &[Event]) -> bool {
    events.iter().any(|ev| matches!(ev, Event::Empty(_)))
}

fn all_tx_results_found(tx_sync_results: &[TxSyncResult]) -> bool {
    tx_sync_results
        .iter()
        .all(|r| !empty_event_present(&r.events))
}

impl Chain for CosmosSdkChain {
    fn bootstrap(config: ChainConfig, rt: Arc<TokioRuntime>) -> Result<Self, Error> {
        let rpc_client = HttpClient::new(config.rpc_addr.clone())
            .map_err(|e| Kind::Rpc(config.rpc_addr.clone()).context(e))?;

        // Initialize key store and load key
        let keybase = KeyRing::new(Store::Test, &config.account_prefix, config.id.as_str())
            .map_err(|e| Kind::KeyBase.context(e))?;

        let grpc_addr =
            Uri::from_str(&config.grpc_addr.to_string()).map_err(|e| Kind::Grpc.context(e))?;

        let chain = Self {
            config,
            rpc_client,
            grpc_addr,
            rt,
            keybase,
            account: None,
        };

        chain.health_checkup();

        Ok(chain)
    }

    fn init_event_monitor(
        &self,
        rt: Arc<TokioRuntime>,
    ) -> Result<(EventReceiver, TxMonitorCmd), Error> {
        crate::time!("init_event_monitor");

        let (mut event_monitor, event_receiver, monitor_tx) = EventMonitor::new(
            self.config.id.clone(),
            self.config.websocket_addr.clone(),
            rt,
        )
        .map_err(Kind::EventMonitor)?;

        event_monitor.subscribe().map_err(Kind::EventMonitor)?;

        thread::spawn(move || event_monitor.run());

        Ok((event_receiver, monitor_tx))
    }

    fn shutdown(self) -> Result<(), Error> {
        Ok(())
    }

    fn id(&self) -> &ChainId {
        &self.config().id
    }

    fn keybase(&self) -> &KeyRing {
        &self.keybase
    }

    fn keybase_mut(&mut self) -> &mut KeyRing {
        &mut self.keybase
    }

    /// Send one or more transactions that include all the specified messages.
    /// The `proto_msgs` are split in transactions such they don't exceed the configured maximum
    /// number of messages per transaction and the maximum transaction size.
    /// Then `send_tx()` is called with each Tx. `send_tx()` determines the fee based on the
    /// on-chain simulation and if this exceeds the maximum gas specified in the configuration file
    /// then it returns error.
    /// TODO - more work is required here for a smarter split maybe iteratively accumulating/ evaluating
    /// msgs in a Tx until any of the max size, max num msgs, max fee are exceeded.
    fn send_msgs(&mut self, proto_msgs: Vec<Any>) -> Result<Vec<Event>, Error> {
        crate::time!("send_msgs");

        if proto_msgs.is_empty() {
            return Ok(vec![]);
        }
        let mut tx_sync_results = vec![];

        let mut n = 0;
        let mut size = 0;
        let mut msg_batch = vec![];
        for msg in proto_msgs.iter() {
            msg_batch.push(msg.clone());
            let mut buf = Vec::new();
            prost::Message::encode(msg, &mut buf).unwrap();
            n += 1;
            size += buf.len();
            if n >= self.max_msg_num() || size >= self.max_tx_size() {
                let events_per_tx = vec![Event::Empty("".to_string()); msg_batch.len()];
                let tx_sync_result = self.send_tx(msg_batch)?;
                tx_sync_results.push(TxSyncResult {
                    response: tx_sync_result,
                    events: events_per_tx,
                });
                n = 0;
                size = 0;
                msg_batch = vec![];
            }
        }
        if !msg_batch.is_empty() {
            let events_per_tx = vec![Event::Empty("".to_string()); msg_batch.len()];
            let tx_sync_result = self.send_tx(msg_batch)?;
            tx_sync_results.push(TxSyncResult {
                response: tx_sync_result,
                events: events_per_tx,
            });
        }

        let tx_sync_results = self.wait_for_block_commits(tx_sync_results)?;

        let events = tx_sync_results
            .into_iter()
            .map(|el| el.events)
            .flatten()
            .collect();

        Ok(events)
    }

    /// Get the account for the signer
    fn get_signer(&mut self) -> Result<String, Error> {
        crate::time!("get_signer");

        // Get the key from key seed file
        let key = self
            .keybase()
            .get_key(&self.config.key_name)
            .map_err(|e| Kind::KeyBase.context(e))?;

        let bech32 = encode_to_bech32(&key.address.to_hex(), &self.config.account_prefix)?;
        Ok(bech32)
    }

    /// Get the signing key
    fn get_key(&mut self) -> Result<KeyEntry, Error> {
        crate::time!("get_key");

        // Get the key from key seed file
        let key = self
            .keybase()
            .get_key(&self.config.key_name)
            .map_err(|e| Kind::KeyBase.context(e))?;

        Ok(key)
    }

    /// This function queries transactions for events matching certain criteria.
    /// 1. Client Update request - returns a vector with at most one update client event
    /// 2. Packet event request - returns at most one packet event for each sequence specified
    ///    in the request.
    ///    Note - there is no way to format the packet query such that it asks for Tx-es with either
    ///    sequence (the query conditions can only be AND-ed).
    ///    There is a possibility to include "<=" and ">=" conditions but it doesn't work with
    ///    string attributes (sequence is emmitted as a string).
    ///    Therefore, for packets we perform one tx_search for each sequence.
    ///    Alternatively, a single query for all packets could be performed but it would return all
    ///    packets ever sent.
    fn query_events_from_txs(&self, request: QueryTxRequest) -> Result<Vec<Event>, Error> {
        crate::time!("query_txs");

        match request {
            QueryTxRequest::Transaction(tx) => {
                let mut response = self
                    .block_on(self.rpc_client.tx_search(
                        tx_hash_query(&tx),
                        false,
                        1,
                        1, // get only the first Tx matching the query
                        Order::Ascending,
                    ))
                    .map_err(|e| Kind::Grpc.context(e))?;

                if response.txs.is_empty() {
                    Ok(vec![])
                } else {
                    let tx = response.txs.remove(0);
                    // TODO implementation
                    Ok(vec![])
                }
            }
        }
    }
}

fn tx_hash_query(request: &QueryTxHash) -> Query {
    tendermint_rpc::query::Query::eq("tx.hash", request.0.to_string())
}

/// Perform a `broadcast_tx_sync`, and return the corresponding deserialized response data.
async fn broadcast_tx_sync(
    chain: &CosmosSdkChain,
    data: Vec<u8>,
) -> Result<Response, anomaly::Error<Kind>> {
    let response = chain
        .rpc_client()
        .broadcast_tx_sync(data.into())
        .await
        .map_err(|e| Kind::Rpc(chain.config.rpc_addr.clone()).context(e))?;

    Ok(response)
}

/// Uses the GRPC client to retrieve the account sequence
async fn query_account(chain: &CosmosSdkChain, address: String) -> Result<BaseAccount, Error> {
    let mut client = crate::cosmos::auth::v1beta1::query_client::QueryClient::connect(
        chain.grpc_addr.clone(),
    )
    .await
    .map_err(|e| Kind::Grpc.context(e))?;

    let request = tonic::Request::new(QueryAccountRequest { address });

    let response = client.account(request).await;

    let base_account = BaseAccount::decode(
        response
            .map_err(|e| Kind::Grpc.context(e))?
            .into_inner()
            .account
            .unwrap()
            .value
            .as_slice(),
    )
    .map_err(|e| Kind::Grpc.context(e))?;

    Ok(base_account)
}

fn encode_to_bech32(address: &str, account_prefix: &str) -> Result<String, Error> {
    let account =
        AccountId::from_str(address).map_err(|_| Kind::InvalidKeyAddress(address.to_string()))?;

    let encoded = bech32::encode(account_prefix, account.to_base32(), Variant::Bech32)
        .map_err(Kind::Bech32Encoding)?;

    Ok(encoded)
}

pub struct TxSyncResult {
    // the broadcast_tx_sync response
    response: Response,
    // the events generated by a Tx once executed
    events: Vec<Event>,
}

fn auth_info_and_bytes(signer_info: SignerInfo, fee: Fee) -> Result<(AuthInfo, Vec<u8>), Error> {
    let auth_info = AuthInfo {
        signer_infos: vec![signer_info],
        fee: Some(fee),
    };

    // A protobuf serialization of a AuthInfo
    let mut auth_buf = Vec::new();
    prost::Message::encode(&auth_info, &mut auth_buf).unwrap();
    Ok((auth_info, auth_buf))
}

fn tx_body_and_bytes(proto_msgs: Vec<Any>) -> Result<(TxBody, Vec<u8>), Error> {
    // Create TxBody
    let body = TxBody {
        messages: proto_msgs.to_vec(),
        memo: "".to_string(),
        timeout_height: 0_u64,
        extension_options: Vec::<Any>::new(),
        non_critical_extension_options: Vec::<Any>::new(),
    };
    // A protobuf serialization of a TxBody
    let mut body_buf = Vec::new();
    prost::Message::encode(&body, &mut body_buf).unwrap();
    Ok((body, body_buf))
}

fn calculate_fee(adjusted_gas_amount: u64, gas_price: &GasPrice) -> Coin {
    let fee_amount = Decimal::from_u64(adjusted_gas_amount).unwrap() * Decimal::from_f64(gas_price.price).unwrap();

    return Coin {
        denom: gas_price.denom.to_string(),
        amount: fee_amount.ceil().to_string(),
    };
}
