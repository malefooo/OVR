use crate::{
    common::{
        block_hash_to_evm_format, block_number_to_height, rollback_to_height,
        tm_proposer_to_evm_format, HashValue,
    },
    ledger::{State, MAIN_BRANCH_NAME},
    rpc::{
        error::new_jsonrpc_error,
        utils::{
            filter_block_logs, remove_branch_by_name, rollback_by_height, tx_to_web3_tx,
            txs_to_web3_txs,
        },
    },
    tx::Tx,
    EvmTx,
};
use byte_slice_cast::AsByteSlice;
use ethereum::TransactionAny;
use ethereum_types::{Bloom, H160, H256, H64, U256, U64};
use jsonrpc_core::{BoxFuture, Result};
use rlp::{Decodable, Rlp};
use serde_json::Value;
use std::result::Result::Err;
use web3_rpc_core::{
    types::{
        Block, BlockNumber, BlockTransactions, Bytes, CallRequest, Filter,
        FilteredParams, Index, Log, Receipt, RichBlock, SyncInfo, SyncStatus,
        Transaction, TransactionRequest, Work,
    },
    EthApi,
};

use super::error;

const BASE_GAS: u64 = 21_000;

pub(crate) struct EthApiImpl {
    pub upstream: String,
    pub state: State,
}

impl EthApi for EthApiImpl {
    fn protocol_version(&self) -> BoxFuture<Result<u64>> {
        Box::pin(async move { Ok(65) })
    }

    fn chain_id(&self) -> BoxFuture<Result<Option<U64>>> {
        let chain_id = self.state.chain_id.get_value();
        Box::pin(async move { Ok(Some(U64::from(chain_id))) })
    }

    fn balance(
        &self,
        address: H160,
        bn: Option<BlockNumber>,
    ) -> BoxFuture<Result<U256>> {
        let new_branch_name =
            match rollback_by_height(bn, None, Some(&self.state.evm), "balance") {
                Ok(name) => name,
                Err(e) => {
                    return Box::pin(async { Err(e) });
                }
            };

        let balance = if let Some(balance) = self.state.evm.OFUEL.accounts.get(&address)
        {
            balance.balance
        } else {
            U256::zero()
        };

        if let Err(e) =
            remove_branch_by_name(new_branch_name, None, Some(&self.state.evm))
        {
            return Box::pin(async { Err(e) });
        }

        Box::pin(async move { Ok(balance) })
    }

    // Low priority, not implemented for now
    fn send_transaction(&self, _: TransactionRequest) -> BoxFuture<Result<H256>> {
        // Cal tendermint send tx.

        Box::pin(async { Ok(H256::default()) })
    }

    fn call(
        &self,
        req: CallRequest,
        bn: Option<BlockNumber>,
    ) -> BoxFuture<Result<Bytes>> {
        let r;
        let resp = self
            .state
            .evm
            .contract_handle(MAIN_BRANCH_NAME, req, bn)
            .map_err(|e| {
                error::new_jsonrpc_error(
                    "call contract failed",
                    Value::String(e.to_string()),
                )
            });

        ruc::d!(format!("{:?}", resp));

        if let Err(e) = resp {
            r = Err(e)
        } else if let Ok(resp) = resp {
            let bytes = Bytes::new(resp.data);
            r = Ok(bytes)
        } else {
            r = Ok(Bytes::default())
        }

        Box::pin(async { r })
    }

    fn syncing(&self) -> BoxFuture<Result<SyncStatus>> {
        // is a syncing fullnode?
        let upstream = self.upstream.clone();

        Box::pin(async move {
            let url = format!("{}/{}", upstream, "status");
            let resp = reqwest::Client::new()
                .get(url)
                .send()
                .await
                .map_err(|e| {
                    error::new_jsonrpc_error("req error", Value::String(e.to_string()))
                })?
                .json::<Value>()
                .await
                .map_err(|e| {
                    error::new_jsonrpc_error(
                        "resp to value error",
                        Value::String(e.to_string()),
                    )
                })?;
            ruc::d!(resp);

            let mut r = Err(error::new_jsonrpc_error(
                "send tx to tendermint failed",
                resp.clone(),
            ));
            if let Some(result) = resp.get("result") {
                // The following unwrap operations are safe
                // If the return result field of tendermint remains unchanged
                let sync_info = result.get("sync_info").unwrap();
                let catching_up =
                    sync_info.get("catching_up").unwrap().as_bool().unwrap();
                if catching_up {
                    r = Ok(SyncStatus::Info(SyncInfo {
                        starting_block: Default::default(),
                        current_block: U256::from(
                            sync_info
                                .get("latest_block_height")
                                .unwrap()
                                .to_string()
                                .as_bytes(),
                        ),
                        highest_block: Default::default(),
                        warp_chunks_amount: None,
                        warp_chunks_processed: None,
                    }))
                } else {
                    r = Ok(SyncStatus::None)
                }
            }

            r
        })
    }

    fn author(&self) -> BoxFuture<Result<H160>> {
        // current proposer
        let current_proposer = self.state.evm.vicinity.block_coinbase;

        Box::pin(async move { Ok(current_proposer) })
    }

    fn is_mining(&self) -> BoxFuture<Result<bool>> {
        // is validator?

        Box::pin(async move { Ok(false) })
    }

    fn gas_price(&self) -> BoxFuture<Result<U256>> {
        let gas_price = self.state.evm.gas_price.get_value();

        Box::pin(async move { Ok(gas_price) })
    }

    fn block_number(&self) -> BoxFuture<Result<U256>> {
        // return current latest block number.

        let r = if let Some((height, _)) = self.state.blocks.last() {
            Ok(U256::from(height))
        } else {
            Err(new_jsonrpc_error("state blocks is none", Value::Null))
        };

        Box::pin(async move { r })
    }

    fn storage_at(
        &self,
        addr: H160,
        index: U256,
        bn: Option<BlockNumber>,
    ) -> BoxFuture<Result<H256>> {
        let new_branch_name =
            match rollback_by_height(bn, None, Some(&self.state.evm), "storage_at") {
                Ok(name) => name,
                Err(e) => {
                    return Box::pin(async { Err(e) });
                }
            };

        // TODO: I'm not sure if this is the right thing to do
        let key = (&addr, &H256::from_slice(index.as_byte_slice()));

        let val = if let Some(val) = self.state.evm.OFUEL.storages.get(&key) {
            val
        } else {
            H256::default()
        };

        if let Err(e) =
            remove_branch_by_name(new_branch_name, None, Some(&self.state.evm))
        {
            return Box::pin(async { Err(e) });
        }

        Box::pin(async move { Ok(val) })
    }

    fn block_by_hash(
        &self,
        block_hash: H256,
        is_complete: bool,
    ) -> BoxFuture<Result<Option<RichBlock>>> {
        let mut op_rb = None;
        for (height, block) in self.state.blocks.iter() {
            let chain_id = self.state.chain_id.get_value();

            if block.header_hash.as_slice() == block_hash.as_bytes() {
                let proposer = tm_proposer_to_evm_format(&block.header.proposer);

                let receipt =
                    if let Some((_, receipt)) = block.header.receipts.iter().last() {
                        receipt.clone()
                    } else {
                        Default::default()
                    };

                // prev is null if block is 1
                let parent_hash = if block.header.prev_hash.is_empty() {
                    H256::default()
                } else {
                    block_hash_to_evm_format(&block.header.prev_hash)
                };

                let web3_txs = match txs_to_web3_txs(&block, chain_id, height) {
                    Ok(v) => v,
                    Err(e) => return Box::pin(async { Err(e) }),
                };

                let mut b = Block {
                    hash: Some(block_hash_to_evm_format(&block.header_hash)),
                    parent_hash,
                    uncles_hash: Default::default(),
                    author: proposer,
                    miner: proposer,
                    state_root: Default::default(),
                    transactions_root: block_hash_to_evm_format(
                        &block.header.tx_merkle.root_hash,
                    ),
                    receipts_root: Default::default(),
                    number: Some(U256::from(height)),
                    gas_used: receipt.block_gas_used,
                    gas_limit: self.state.evm.block_gas_limit.get_value(),
                    extra_data: Default::default(),
                    logs_bloom: Some(Bloom::from_slice(block.bloom.as_slice())),
                    timestamp: U256::from(block.header.timestamp),
                    difficulty: Default::default(),
                    total_difficulty: Default::default(),
                    seal_fields: vec![],
                    uncles: vec![],
                    transactions: BlockTransactions::Full(web3_txs.clone()),
                    size: Some(U256::from(
                        serde_json::to_vec(&block).unwrap_or_default().len(),
                    )),
                };

                // Determine if you want to return all block information
                if is_complete {
                    let tx_hashes =
                        web3_txs.iter().map(|t| t.hash).collect::<Vec<H256>>();
                    b.transactions = BlockTransactions::Hashes(tx_hashes);
                }

                op_rb.replace(RichBlock {
                    inner: b,
                    extra_info: Default::default(),
                });
            }
        }

        Box::pin(async { Ok(op_rb) })
    }

    fn block_by_number(
        &self,
        bn: BlockNumber,
        is_complete: bool,
    ) -> BoxFuture<Result<Option<RichBlock>>> {
        let height = block_number_to_height(Some(bn), None, Some(&self.state.evm));

        let new_branch_name = match rollback_to_height(
            height,
            None,
            Some(&self.state.evm),
            "block_by_number",
        ) {
            Ok(name) => name,
            Err(e) => {
                return Box::pin(async move {
                    Err(new_jsonrpc_error(
                        "rollback to height",
                        Value::String(e.to_string()),
                    ))
                });
            }
        };

        let op = if let Some(block) = self.state.blocks.get(&height) {
            let proposer = tm_proposer_to_evm_format(&block.header.proposer);

            // prev is null if block is 1
            let parent_hash = if block.header.prev_hash.is_empty() {
                H256::default()
            } else {
                block_hash_to_evm_format(&block.header.prev_hash)
            };

            let receipt = if let Some((_, receipt)) = block.header.receipts.iter().last()
            {
                receipt.clone()
            } else {
                Default::default()
            };

            let chain_id = self.state.chain_id.get_value();
            let web3_txs = match txs_to_web3_txs(&block, chain_id, height) {
                Ok(v) => v,
                Err(e) => {
                    // You must delete the branch before returning,
                    // otherwise the next time you come in,
                    // the same branch will exist and an error will be reported.
                    if let Err(e) = remove_branch_by_name(
                        new_branch_name,
                        None,
                        Some(&self.state.evm),
                    ) {
                        return Box::pin(async { Err(e) });
                    }

                    return Box::pin(async { Err(e) });
                }
            };

            let mut b = Block {
                hash: Some(block_hash_to_evm_format(&block.header_hash)),
                parent_hash,
                uncles_hash: Default::default(),
                author: proposer,
                miner: proposer,
                state_root: Default::default(),
                transactions_root: block_hash_to_evm_format(
                    &block.header.tx_merkle.root_hash,
                ),
                receipts_root: Default::default(),
                number: Some(U256::from(height)),
                gas_used: receipt.block_gas_used,
                gas_limit: self.state.evm.block_gas_limit.get_value(),
                extra_data: Default::default(),
                logs_bloom: Some(Bloom::from_slice(&block.bloom)),
                timestamp: U256::from(block.header.timestamp),
                difficulty: Default::default(),
                total_difficulty: Default::default(),
                seal_fields: vec![],
                uncles: vec![],
                transactions: BlockTransactions::Full(web3_txs.clone()),
                size: Some(U256::from(
                    serde_json::to_vec(&block).unwrap_or_default().len(),
                )),
            };

            if !is_complete {
                let tx_hashes = web3_txs.iter().map(|t| t.hash).collect::<Vec<H256>>();
                b.transactions = BlockTransactions::Hashes(tx_hashes);
            }

            Some(RichBlock {
                inner: b,
                extra_info: Default::default(),
            })
        } else {
            None
        };

        if let Err(e) =
            remove_branch_by_name(new_branch_name, None, Some(&self.state.evm))
        {
            return Box::pin(async { Err(e) });
        }

        Box::pin(async { Ok(op) })
    }

    fn transaction_count(
        &self,
        addr: H160,
        bn: Option<BlockNumber>,
    ) -> BoxFuture<Result<U256>> {
        let height = block_number_to_height(bn, Some(&self.state), None);
        let new_branch_name = match rollback_to_height(
            height,
            Some(&self.state),
            None,
            "transaction_count",
        ) {
            Ok(name) => name,
            Err(e) => {
                return Box::pin(async move {
                    Err(new_jsonrpc_error(
                        "rollback to height",
                        Value::String(e.to_string()),
                    ))
                });
            }
        };

        let mut nonce = U256::zero();

        if let Some(account) = self.state.evm.OFUEL.accounts.get(&addr) {
            nonce = account.nonce
        }

        if let Err(e) = remove_branch_by_name(new_branch_name, Some(&self.state), None) {
            return Box::pin(async { Err(e) });
        }

        Box::pin(async move { Ok(nonce) })
    }

    fn block_transaction_count_by_hash(
        &self,
        block_hash: H256,
    ) -> BoxFuture<Result<Option<U256>>> {
        let mut tx_count = 0;

        for (_, block) in self.state.blocks.iter() {
            if block.header_hash == block_hash.as_bytes() {
                tx_count = block.txs.len();
            }
        }

        Box::pin(async move { Ok(Some(U256::from(tx_count))) })
    }

    fn block_transaction_count_by_number(
        &self,
        bn: BlockNumber,
    ) -> BoxFuture<Result<Option<U256>>> {
        let height = block_number_to_height(Some(bn), Some(&self.state), None);

        let new_branch_name = match rollback_to_height(
            height,
            Some(&self.state),
            None,
            "block_transaction_count_by_number",
        ) {
            Ok(name) => name,
            Err(e) => {
                return Box::pin(async move {
                    Err(new_jsonrpc_error(
                        "rollback to height",
                        Value::String(e.to_string()),
                    ))
                });
            }
        };

        let tx_count = if let Some(block) = self.state.blocks.get(&height) {
            block.txs.len()
        } else {
            Default::default()
        };

        if let Err(e) =
            remove_branch_by_name(new_branch_name, None, Some(&self.state.evm))
        {
            return Box::pin(async { Err(e) });
        }

        Box::pin(async move { Ok(Some(U256::from(tx_count))) })
    }

    fn code_at(&self, addr: H160, bn: Option<BlockNumber>) -> BoxFuture<Result<Bytes>> {
        let new_branch_name =
            match rollback_by_height(bn, None, Some(&self.state.evm), "code_at") {
                Ok(name) => name,
                Err(e) => {
                    return Box::pin(async { Err(e) });
                }
            };

        let bytes = if let Some(account) = self.state.evm.OFUEL.accounts.get(&addr) {
            account.code
        } else {
            Default::default()
        };

        if let Err(e) =
            remove_branch_by_name(new_branch_name, None, Some(&self.state.evm))
        {
            return Box::pin(async { Err(e) });
        }

        Box::pin(async { Ok(Bytes::new(bytes)) })
    }

    fn send_raw_transaction(&self, tx: Bytes) -> BoxFuture<Result<H256>> {
        let evm_tx = match TransactionAny::decode(&Rlp::new(tx.0.as_slice())) {
            Ok(t) => t,
            Err(e) => {
                return Box::pin(async move {
                    Err(new_jsonrpc_error(
                        "bytes decode to transaction2 error",
                        Value::String(e.to_string()),
                    ))
                });
            }
        };

        let tx = Tx::Evm(EvmTx { tx: evm_tx });
        let bytes = match serde_json::to_vec(&tx) {
            Ok(b) => b,
            Err(e) => {
                return Box::pin(async move {
                    Err(new_jsonrpc_error(
                        "tx to bytes error",
                        Value::String(e.to_string()),
                    ))
                });
            }
        };

        let upstream = self.upstream.clone();
        Box::pin(async move {
            let tx_base64 = base64::encode(bytes);
            let json_rpc = format!(
                "{{\"jsonrpc\":\"2.0\",\"id\":\"anything\",\"method\":\"broadcast_tx_sync\",\"params\": {{\"tx\": \"{}\"}}}}",
                &tx_base64
            );

            let resp = reqwest::Client::new()
                .post(upstream)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(json_rpc)
                .send()
                .await
                .map_err(|e| {
                    error::new_jsonrpc_error("req error", Value::String(e.to_string()))
                })?
                .json::<Value>()
                .await
                .map_err(|e| {
                    error::new_jsonrpc_error(
                        "resp to value error",
                        Value::String(e.to_string()),
                    )
                })?;

            ruc::d!(resp);
            let mut r = Err(error::new_jsonrpc_error(
                "send tx to tendermint failed",
                resp.clone(),
            ));
            if let Some(result) = resp.get("result") {
                if let Some(code) = result.get("code") {
                    if code.eq(&0) {
                        r = Ok(block_hash_to_evm_format(&tx.hash()))
                    }
                }
            }

            r
        })
    }

    fn estimate_gas(
        &self,
        req: CallRequest,
        bn: Option<BlockNumber>,
    ) -> BoxFuture<Result<U256>> {
        let r;
        let resp = self
            .state
            .evm
            .contract_handle(MAIN_BRANCH_NAME, req, bn)
            .map_err(|e| {
                error::new_jsonrpc_error(
                    "call contract failed",
                    Value::String(e.to_string()),
                )
            });

        ruc::d!(format!("{:?}", resp));

        if let Err(e) = resp {
            r = Err(e)
        } else if let Ok(resp) = resp {
            let gas_used = U256::from(resp.gas_used + BASE_GAS);
            r = Ok(gas_used)
        } else {
            r = Err(new_jsonrpc_error("call contract resp none", Value::Null));
        }

        Box::pin(async { r })
    }

    fn transaction_by_hash(
        &self,
        tx_hash: H256,
    ) -> BoxFuture<Result<Option<Transaction>>> {
        let mut transaction = None;

        'outer: for (height, block) in self.state.blocks.iter() {
            for (index, tx) in block.txs.iter().enumerate() {
                if tx.hash() == tx_hash.as_bytes() {
                    match tx_to_web3_tx(
                        &tx,
                        &block,
                        height,
                        index,
                        self.state.chain_id.get_value(),
                    ) {
                        Ok(op) => {
                            transaction = op;
                            break 'outer;
                        }
                        Err(e) => {
                            return Box::pin(async { Err(e) });
                        }
                    }
                }
            }
        }

        Box::pin(async { Ok(transaction) })
    }

    fn transaction_by_block_hash_and_index(
        &self,
        block_hash: H256,
        index: Index,
    ) -> BoxFuture<Result<Option<Transaction>>> {
        let mut transaction = None;

        for (height, block) in self.state.blocks.iter() {
            if block.header_hash == block_hash.as_bytes() {
                if let Some(tx) = block.txs.get(index.value()) {
                    match tx_to_web3_tx(
                        &tx,
                        &block,
                        height,
                        index.value(),
                        self.state.chain_id.get_value(),
                    ) {
                        Ok(op) => {
                            transaction = op;
                            break;
                        }
                        Err(e) => {
                            return Box::pin(async { Err(e) });
                        }
                    }
                }
            }
        }

        Box::pin(async { Ok(transaction) })
    }

    fn transaction_by_block_number_and_index(
        &self,
        bn: BlockNumber,
        index: Index,
    ) -> BoxFuture<Result<Option<Transaction>>> {
        let height = block_number_to_height(Some(bn), Some(&self.state), None);
        let new_branch_name = match rollback_to_height(
            height,
            Some(&self.state),
            None,
            "transaction_by_block_number_and_index",
        ) {
            Ok(name) => name,
            Err(e) => {
                return Box::pin(async move {
                    Err(new_jsonrpc_error(
                        "rollback to height",
                        Value::String(e.to_string()),
                    ))
                });
            }
        };

        let mut transaction = None;

        if let Some(block) = self.state.blocks.get(&height) {
            if let Some(tx) = block.txs.get(index.value()) {
                match tx_to_web3_tx(
                    &tx,
                    &block,
                    height,
                    index.value(),
                    self.state.chain_id.get_value(),
                ) {
                    Ok(op) => {
                        transaction = op;
                    }
                    Err(e) => {
                        return Box::pin(async { Err(e) });
                    }
                }
            }
        }

        if let Err(e) =
            remove_branch_by_name(new_branch_name, None, Some(&self.state.evm))
        {
            return Box::pin(async { Err(e) });
        }

        Box::pin(async { Ok(transaction) })
    }

    fn transaction_receipt(&self, tx_hash: H256) -> BoxFuture<Result<Option<Receipt>>> {
        let mut op = None;

        for (height, block) in self.state.blocks.iter() {
            let hash = HashValue::from(tx_hash.as_bytes());
            let block_hash = block_hash_to_evm_format(&block.header_hash);

            if let Some(r) = block.header.receipts.get(&hash) {
                let mut logs = vec![];

                for l in r.logs.iter() {
                    logs.push(Log {
                        address: l.address,
                        topics: l.topics.clone(),
                        data: Bytes::new(l.data.clone()),
                        block_hash: Some(block_hash),
                        block_number: Some(U256::from(height)),
                        transaction_hash: Some(tx_hash),
                        transaction_index: Some(U256::from(l.tx_index)),
                        log_index: Some(U256::from(l.log_index_in_block)),
                        transaction_log_index: Some(U256::from(l.log_index_in_tx)),
                        removed: false,
                    });
                }

                op.replace(Receipt {
                    transaction_hash: Some(tx_hash),
                    transaction_index: Some(U256::from(r.tx_index)),
                    block_hash: Some(block_hash),
                    from: r.from,
                    to: r.to,
                    block_number: Some(U256::from(height)),
                    cumulative_gas_used: r.block_gas_used,
                    gas_used: Some(r.tx_gas_used),
                    contract_address: r.contract_addr,
                    logs,
                    state_root: None,
                    logs_bloom: Default::default(),
                    status_code: None,
                });
            }
        }

        Box::pin(async { Ok(op) })
    }

    fn logs(&self, filter: Filter) -> BoxFuture<Result<Vec<Log>>> {
        let mut logs = vec![];

        if let Some(hash) = filter.block_hash {
            for (height, block) in self.state.blocks.iter() {
                if block.header_hash == hash.as_bytes() {
                    logs.append(&mut filter_block_logs(&block, &filter, height));
                    break;
                }
            }
        } else {
            let (current_height, _) = self.state.blocks.last().unwrap_or_default();

            let mut to =
                block_number_to_height(filter.to_block.clone(), Some(&self.state), None);
            if to > current_height {
                to = current_height;
            }

            let mut from = block_number_to_height(
                filter.from_block.clone(),
                Some(&self.state),
                None,
            );
            if from > current_height {
                from = current_height;
            }

            let topics_input = if filter.topics.is_some() {
                let filtered_params = FilteredParams::new(Some(filter.clone()));
                Some(filtered_params.flat_topics)
            } else {
                None
            };

            let address_bloom_filter =
                FilteredParams::addresses_bloom_filter(&filter.address);
            let topic_bloom_filters = FilteredParams::topics_bloom_filter(&topics_input);

            for height in from..=to {
                if let Some(block) = self.state.blocks.get(&height) {
                    let b = Bloom::from_slice(block.bloom.as_slice());
                    if FilteredParams::address_in_bloom(b, &address_bloom_filter)
                        && FilteredParams::topics_in_bloom(b, &topic_bloom_filters)
                    {
                        logs.append(&mut filter_block_logs(&block, &filter, height));
                    }
                }
            }
        };

        Box::pin(async { Ok(logs) })
    }

    // ----------- Not impl.
    fn work(&self) -> Result<Work> {
        Err(error::no_impl())
    }

    fn submit_work(&self, _: H64, _: H256, _: H256) -> Result<bool> {
        Err(error::no_impl())
    }

    fn submit_hashrate(&self, _: U256, _: H256) -> Result<bool> {
        Err(error::no_impl())
    }

    fn hashrate(&self) -> Result<U256> {
        Err(error::no_impl())
    }
    fn uncle_by_block_hash_and_index(
        &self,
        _: H256,
        _: Index,
    ) -> Result<Option<RichBlock>> {
        Err(error::no_impl())
    }

    fn uncle_by_block_number_and_index(
        &self,
        _: BlockNumber,
        _: Index,
    ) -> Result<Option<RichBlock>> {
        Err(error::no_impl())
    }

    fn block_uncles_count_by_hash(&self, _: H256) -> Result<U256> {
        Err(error::no_impl())
    }

    fn block_uncles_count_by_number(&self, _: BlockNumber) -> Result<U256> {
        Err(error::no_impl())
    }

    fn accounts(&self) -> Result<Vec<H160>> {
        // This api is no impl, only return a empty array.
        Ok(vec![])
    }
}
