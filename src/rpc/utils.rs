use crate::{
    common::{BlockHeight},
    ledger::Block,
    rpc::error::new_jsonrpc_error,
    tx::Tx,
};
use ethereum_types::{H256, U256, U64};
use primitive_types::H512;
use rustc_hex::ToHex;
use serde_json::Value;
use web3_rpc_core::types::{
    Bytes, Filter, FilteredParams, Log as Web3Log, Transaction,
};

pub fn txs_to_web3_txs(
    block: &Block,
    chain_id: u64,
    height: BlockHeight,
) -> jsonrpc_core::Result<Vec<Transaction>> {
    let mut web3_txs = vec![];
    for (tx_index, tx) in block.txs.iter().enumerate() {
        match tx_to_web3_tx(&tx, block, height, tx_index, chain_id) {
            Ok(op) => {
                if let Some(t) = op {
                    web3_txs.push(t);
                }
            }
            Err(e) => {
                return Err(e);
            }
        }
    }
    Ok(web3_txs)
}

pub fn tx_to_web3_tx(
    tx: &Tx,
    block: &Block,
    height: BlockHeight,
    index: usize,
    chain_id: u64,
) -> jsonrpc_core::Result<Option<Transaction>> {
    let op = match &tx {
        Tx::Evm(evm_tx) => {
            let tx_properties = evm_tx.get_tx_common_properties();
            let (from, to) = evm_tx.get_from_to();
            let public_key = evm_tx
                .recover_pubkey()
                .map(|pubkey| H512::from_slice(pubkey.as_slice()));
            let raw = match serde_json::to_vec(&tx) {
                Ok(v) => Bytes::new(v),
                Err(e) => {
                    return Err(new_jsonrpc_error(
                        "tx to bytes error",
                        Value::String(e.to_string()),
                    ));
                }
            };

            let from = if let Some(from) = from {
                from
            } else {
                return Err(new_jsonrpc_error(
                    "The transaction has no originator",
                    Value::String(tx.hash().to_hex()),
                ));
            };

            let receipt = if let Some(receipt) = block.header.receipts.get(&tx.hash()) {
                receipt
            } else {
                return Err(new_jsonrpc_error(
                    "The transaction has no receipt",
                    Value::String(tx.hash().to_hex()),
                ));
            };

            Some(Transaction {
                hash: H256::from_slice(tx.hash().as_slice()),
                nonce: tx_properties.nonce,
                block_hash: Some(H256::from_slice(block.header_hash.as_slice())),
                block_number: Some(U256::from(height)),
                transaction_index: Some(U256::from(index)),
                from,
                to,
                value: tx_properties.value,
                gas_price: tx_properties.gas_price,
                gas: tx_properties
                    .gas_limit
                    .saturating_mul(tx_properties.gas_price),
                input: Bytes::new(tx_properties.input),
                creates: receipt.contract_addr,
                raw,
                public_key,
                chain_id: Some(U64::from(chain_id)),
                standard_v: U256::from(tx_properties.v),
                v: U256::from(tx_properties.v),
                r: U256::from(tx_properties.r.as_bytes()),
                s: U256::from(tx_properties.s.as_bytes()),
            })
        }
        // TODO: Native trading to be achieved
        Tx::Native(_) => None,
    };

    Ok(op)
}

pub fn filter_block_logs(
    block: &Block,
    filter: &Filter,
    height: BlockHeight,
) -> Vec<Web3Log> {
    let mut logs = vec![];

    let params = FilteredParams::new(Some(filter.clone()));

    for (tx_hash, receipt) in block.header.receipts.iter() {
        for l in receipt.logs.iter() {
            let log = Web3Log {
                address: l.address,
                topics: l.topics.clone(),
                data: Bytes::new(l.data.clone()),
                block_hash: Some(H256::from_slice(block.header_hash.as_slice())),
                block_number: Some(U256::from(height)),
                transaction_hash: Some(H256::from_slice(tx_hash.as_slice())),
                transaction_index: Some(U256::from(l.tx_index)),
                log_index: Some(U256::from(l.log_index_in_block)),
                transaction_log_index: Some(U256::from(l.log_index_in_tx)),
                removed: false,
            };

            let mut add = true;

            match (filter.address.clone(), filter.topics.clone()) {
                (Some(_), Some(_)) => {
                    if !params.filter_address(&log) || !params.filter_topics(&log) {
                        add = false;
                    }
                }
                (Some(_), None) => {
                    if !params.filter_address(&log) {
                        add = false;
                    }
                }
                (None, Some(_)) => {
                    if !params.filter_topics(&log) {
                        add = false;
                    }
                }
                (None, None) => {}
            }

            if add {
                logs.push(log);
            }
        }
    }
    logs
}
