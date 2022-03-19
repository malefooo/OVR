pub mod impls;
mod precompile;
pub mod tx;

use crate::{
    common::{block_number_to_height, BlockHeight},
    ethvm::{impls::stack::OvrStackState, precompile::PRECOMPILE_SET},
    ledger::{VsVersion, MAIN_BRANCH_NAME},
};
use evm::{
    executor::stack::{StackExecutor, StackSubstateMetadata},
    ExitReason,
};
use impls::backend::OvrBackend;
use once_cell::sync::Lazy;
use primitive_types::{H160, H256, U256};
use ruc::*;
use serde::{Deserialize, Serialize};
use tx::token::Erc20Like;
use vsdb::{
    basic::orphan::Orphan, BranchName, MapxOrd, OrphanVs, ParentBranchName, ValueEn, Vs,
    VsMgmt,
};
use web3_rpc_core::types::{BlockNumber, CallRequest};

#[allow(non_snake_case)]
#[derive(Vs, Clone, Debug, Deserialize, Serialize)]
pub struct State {
    pub gas_price: OrphanVs<U256>,
    pub block_gas_limit: OrphanVs<U256>,
    pub block_base_fee_per_gas: OrphanVs<U256>,

    pub OFUEL: Erc20Like,

    // Environmental block hashes.
    pub block_hashes: MapxOrd<BlockHeight, H256>,

    // Oneshot values for each evm transaction.
    pub vicinity: OvrVicinity,
}

impl State {
    pub fn contract_handle(
        &self,
        branch_name: BranchName,
        req: CallRequest,
        bn: Option<BlockNumber>,
    ) -> Result<CallContractResp> {
        static U64_MAX: Lazy<U256> = Lazy::new(|| U256::from(u64::MAX));

        // Operation Type
        enum Operation {
            Call,
            Create,
        }

        // Determine what type of operation is being performed based on the parameter to in the request object
        let (operation, address) = if let Some(to) = req.to {
            (Operation::Call, to)
        } else {
            (Operation::Create, H160::default())
        };

        let caller = req.from.unwrap_or_default();
        let value = req.value.unwrap_or_default();
        let data = req.data.unwrap_or_default();

        // This parameter is used as the divisor and cannot be 0
        let gas = if let Some(gas) = req.gas {
            alt!(gas > *U64_MAX, u64::MAX, gas.as_u64())
        } else {
            u64::MAX
        };
        let gas_price = req.gas_price.unwrap_or_else(U256::one);
        let gas_price = alt!(gas_price > *U64_MAX, u64::MAX, gas_price.as_u64());
        let gas_limit = gas.checked_div(gas_price).unwrap(); //safe

        let height = block_number_to_height(bn, None, Some(self));

        let branch_tmp = snapshot_at_height(height, self, "call_contract")?;

        let backend = OvrBackend {
            branch: branch_name,
            state: self.OFUEL.accounts.clone(),
            storages: self.OFUEL.storages.clone(),
            block_hashes: self.block_hashes,
            vicinity: self.vicinity.clone(),
        };

        let cfg = evm::Config::istanbul();
        let metadata = StackSubstateMetadata::new(u64::MAX, &cfg);

        let ovr_stack_state = OvrStackState::new(metadata, &backend);
        let precompiles = PRECOMPILE_SET.clone();
        let mut executor =
            StackExecutor::new_with_precompiles(ovr_stack_state, &cfg, &precompiles);

        let resp = match operation {
            Operation::Call => {
                executor.transact_call(caller, address, value, data.0, gas_limit, vec![])
            }
            Operation::Create => (
                executor.transact_create(caller, value, data.0, gas_limit, vec![]),
                vec![],
            ),
        };

        d!(format!("{:?}", resp));

        let cc_resp = CallContractResp {
            evm_resp: resp.0,
            data: resp.1,
            gas_used: executor.used_gas(),
        };

        self.branch_remove(BranchName::from(branch_tmp.as_str()))?;

        Ok(cc_resp)
    }

    #[inline(always)]
    fn get_backend_hdr<'a>(&self, branch: BranchName<'a>) -> OvrBackend<'a> {
        OvrBackend {
            branch,
            state: self.OFUEL.accounts.clone(),
            storages: self.OFUEL.storages.clone(),
            block_hashes: self.block_hashes,
            vicinity: self.vicinity.clone(),
        }
    }

    // update with each new block
    #[inline(always)]
    pub fn update_vicinity(
        &mut self,
        chain_id: U256,
        block_coinbase: H160,
        block_timestamp: U256,
    ) {
        self.vicinity = OvrVicinity {
            gas_price: self.gas_price.get_value(),
            origin: H160::zero(),
            chain_id,
            block_number: U256::from(
                self.block_hashes.last().map(|(h, _)| h).unwrap_or(0),
            ),
            block_coinbase,
            block_timestamp,
            block_difficulty: U256::zero(),
            block_gas_limit: self.block_gas_limit.get_value(),
            block_base_fee_per_gas: self.block_base_fee_per_gas.get_value(),
        };
    }
}

impl Default for State {
    // NOTE:
    // Do NOT use `..Default::default()` style!
    // Using this style here will make your stack overflow.
    fn default() -> Self {
        Self {
            gas_price: OrphanVs::default(),
            block_gas_limit: OrphanVs::default(),
            block_base_fee_per_gas: OrphanVs::default(),
            OFUEL: Erc20Like::ofuel_token(),
            block_hashes: MapxOrd::new(),
            vicinity: OvrVicinity::default(),
        }
    }
}

// Account information of a vsdb backend.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct OvrAccount {
    pub nonce: U256,
    pub balance: U256,
    pub code: Vec<u8>,
}

impl OvrAccount {
    pub fn from_balance(balance: U256) -> Self {
        Self {
            balance,
            ..Default::default()
        }
    }
}

#[derive(Vs, Default, Clone, Debug, Serialize, Deserialize)]
pub struct OvrVicinity {
    pub gas_price: U256,
    pub origin: H160,
    pub chain_id: U256,
    // Environmental block number.
    pub block_number: U256,
    // Environmental coinbase.
    // `H160(original proposer address)`
    pub block_coinbase: H160,
    // Environmental block timestamp.
    pub block_timestamp: U256,
    // Environmental block difficulty.
    pub block_difficulty: U256,
    // Environmental block gas limit.
    pub block_gas_limit: U256,
    // Environmental base fee per gas.
    pub block_base_fee_per_gas: U256,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CallContractResp {
    pub evm_resp: ExitReason,
    pub data: Vec<u8>,
    pub gas_used: u64,
}

fn snapshot_at_height(
    height: BlockHeight,
    evm_state: &State,
    prefix: &str,
) -> Result<String> {
    static TMP_ID: Lazy<Orphan<u64>> = Lazy::new(Orphan::default);

    if height > 0 {
        let ver = VsVersion::new_with_default_mark(height, u64::MAX);

        let mut id = TMP_ID.get_mut();
        let ver_tmp = format!("{}_{}_{}", prefix, height, *id);
        let branch_tmp = format!("{}_{}_{}", prefix, height, *id);
        *id += 1;

        evm_state.branch_create_by_base_branch_version(
            BranchName::from(branch_tmp.as_str()),
            ParentBranchName::from(MAIN_BRANCH_NAME.0),
            ver.encode_value().as_ref().into(),
        )?;

        evm_state.version_create_by_branch(
            ver_tmp.encode_value().as_ref().into(),
            BranchName::from(branch_tmp.as_str()),
        )?;

        Ok(branch_tmp)
    } else {
        Err(eg!("block height cannot be 0"))
    }
}
