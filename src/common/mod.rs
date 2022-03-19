use std::collections::BTreeMap;

use crate::{
    ledger::Log,
    {ethvm::State as EvmState, ledger::State as LedgerState},
};
use ethereum_types::{Bloom, BloomInput};
use primitive_types::{H160, H256, U256};
use ruc::*;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use web3_rpc_core::types::BlockNumber;

pub(crate) type BlockHeight = u64;

pub(crate) type HashValue = Vec<u8>;
pub(crate) type HashValueRef<'a> = &'a [u8];

pub(crate) type TmAddress = Vec<u8>;
pub(crate) type TmAddressRef<'a> = &'a [u8];

/// global hash function
pub fn hash_sha3_256(contents: &[&[u8]]) -> Vec<u8> {
    let mut hasher = Sha3_256::new();
    for c in contents {
        hasher.update(c);
    }
    hasher.finalize().to_vec()
}

/// block proposer address of tendermint ==> evm coinbase address
pub fn tm_proposer_to_evm_format(addr: TmAddressRef) -> H160 {
    const LEN: usize = H160::len_bytes();

    let mut buf = [0_u8; LEN];
    buf.copy_from_slice(&addr[..min!(LEN, addr.len())]);

    H160::from_slice(&buf)
}

/// block proposer address of tendermint ==> evm coinbase address
pub fn block_hash_to_evm_format(hash: &HashValue) -> H256 {
    const LEN: usize = H256::len_bytes();

    let mut buf = [0; LEN];
    buf.copy_from_slice(&hash[..min!(LEN, hash.len())]);

    H256::from_slice(&buf)
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct InitalContract {
    pub from: H160,
    pub salt: String,
    pub bytecode: String,
}

impl InitalContract {
    pub fn new(from: H160, salt: String) -> Self {
        Self {
            from,
            salt,
            bytecode: String::new(),
        }
    }
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct InitalState {
    pub addr_to_amount: BTreeMap<H160, U256>,
    pub inital_contracts: Vec<InitalContract>,
}

pub fn block_number_to_height(
    bn: Option<BlockNumber>,
    ledger_state: Option<&LedgerState>,
    evm_state: Option<&EvmState>,
) -> BlockHeight {
    let bn = if let Some(bn) = bn {
        bn
    } else {
        BlockNumber::Latest
    };

    match bn {
        BlockNumber::Hash {
            hash,
            require_canonical: _,
        } => {
            let mut h = 0;
            if let Some(evm_state) = evm_state {
                for (height, block_hash) in evm_state.block_hashes.iter() {
                    if block_hash == hash {
                        h = height;
                        break;
                    }
                }
            } else if let Some(ledger_state) = ledger_state {
                for (height, block) in ledger_state.blocks.iter() {
                    if block.header_hash == hash.as_bytes() {
                        h = height;
                        break;
                    }
                }
            }

            h
        }
        BlockNumber::Num(num) => num,
        BlockNumber::Latest => {
            let mut h = 0;

            if let Some(evm_state) = evm_state {
                if let Some((height, _)) = evm_state.block_hashes.iter().last() {
                    h = height;
                }
            } else if let Some(ledger_state) = ledger_state {
                if let Some((height, _)) = ledger_state.blocks.iter().last() {
                    h = height;
                }
            }

            h
        }
        BlockNumber::Earliest => 1,
        BlockNumber::Pending => 0,
    }
}

pub fn handle_bloom(b: &mut Bloom, logs: &[Log]) {
    for log in logs.iter() {
        b.accrue(BloomInput::Raw(&log.address[..]));
        for topic in &log.topics {
            b.accrue(BloomInput::Raw(&topic[..]));
        }
    }
}
