pub mod token;

use crate::{
    common::HashValueRef,
    ethvm::{impls::stack::OvrStackState, precompile::PRECOMPILE_SET, OvrAccount},
    ledger::{Log as LedgerLog, Receipt, StateBranch},
    InitalContract,
};
use ethereum::{Log, TransactionAction, TransactionAny};
use evm::{
    backend::{Apply, ApplyBackend},
    executor::stack::{StackExecutor, StackSubstateMetadata},
    Config as EvmCfg, CreateScheme, ExitReason,
};
use once_cell::sync::Lazy;
use primitive_types::{H160, H256, U256};
use ruc::*;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use std::{collections::BTreeMap, fmt, result::Result as StdResult};
use vsdb::BranchName;

pub static GAS_PRICE_MIN: Lazy<U256> = Lazy::new(|| U256::from(10u8));

type NeededAmount = U256;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Tx {
    pub tx: TransactionAny,
}

impl Tx {
    #[inline(always)]
    pub fn apply(
        self,
        sb: &mut StateBranch,
        b: BranchName,
        estimate: bool,
    ) -> StdResult<(ExecRet, Receipt), Option<ExecRet>> {
        if let Ok((addr, _, gas_price)) = info!(self.pre_exec(sb, b)) {
            let (from, to) = self.get_from_to();
            let ret = self.exec(addr, sb, b, gas_price, estimate);
            let r = ret.gen_receipt(from, to);
            alt!(ret.success, Ok((ret, r)), Err(Some(ret)))
        } else {
            Err(None)
        }
    }

    // 0. ensure the given gas price is big enough
    // 1. verify the transaction signature
    // 2. ensure the transaction nonce is bigger than the last nonce
    // 3. ensure the balance of OFUEL is bigger than `spent_amount + gas_limit`
    // 4. deducte `gas_limit` from the balance of OFUEL
    fn pre_exec(
        &self,
        sb: &mut StateBranch,
        b: BranchName,
    ) -> Result<(H160, OvrAccount, U256)> {
        // {0.}
        let gas_price = self.check_gas_price(sb, b).c(d!())?;

        // {1.} if success, then the transaction signature is valid.
        let addr = self.recover_signer().c(d!())?;

        // {2.}
        if let Err((tx_nonce, system_nonce)) = self.check_nonce(&addr, sb, b) {
            return Err(eg!(
                "Invalid nonce: {}, should be: {}",
                tx_nonce,
                system_nonce
            ));
        }

        // {3.}{4.}
        match self.check_balance(&addr, gas_price, sb, b) {
            Ok((account, _)) => Ok((addr, account, gas_price)),
            Err(Some((account, needed_amount))) => Err(eg!(
                "Insufficient balance, needed: {}, total: {}",
                needed_amount,
                account.balance
            )),
            Err(_) => Err(eg!()),
        }
    }

    // Support:
    // - Legacy transactions
    // - EIP2930 transactons
    // - EIP1559 transactions
    fn exec(
        self,
        addr: H160,
        sb: &mut StateBranch,
        b: BranchName,
        gas_price: U256,
        estimate: bool,
    ) -> ExecRet {
        let mut evm_cfg = EvmCfg::istanbul();
        alt!(estimate, evm_cfg.estimate = true);

        let metadata = StackSubstateMetadata::new(u64::MAX, &evm_cfg);
        let mut backend = sb.state.evm.get_backend_hdr(b);
        let state = OvrStackState::new(metadata, &backend);

        let precompiles = PRECOMPILE_SET.clone();
        let mut executor =
            StackExecutor::new_with_precompiles(state, &evm_cfg, &precompiles);

        let contract_addr;
        let (exit_reason, extra_data) = match self.tx {
            TransactionAny::Legacy(tx) => {
                let gas_limit = tx.gas_limit.try_into().unwrap_or(u64::MAX);
                match tx.action {
                    TransactionAction::Call(target) => {
                        contract_addr = target;
                        executor.transact_call(
                            addr,
                            target,
                            tx.value,
                            tx.input,
                            gas_limit,
                            vec![],
                        )
                    }
                    TransactionAction::Create => {
                        let scheme = CreateScheme::Legacy { caller: addr };
                        contract_addr = executor.create_address(scheme);
                        (
                            executor.transact_create(
                                addr,
                                tx.value,
                                tx.input,
                                gas_limit,
                                vec![],
                            ),
                            vec![],
                        )
                    }
                }
            }
            TransactionAny::EIP2930(tx) => {
                let gas_limit = tx.gas_limit.try_into().unwrap_or(u64::MAX);
                let al = tx
                    .access_list
                    .into_iter()
                    .map(|al| (al.address, al.slots))
                    .collect();
                match tx.action {
                    TransactionAction::Call(target) => {
                        contract_addr = target;
                        executor.transact_call(
                            addr, target, tx.value, tx.input, gas_limit, al,
                        )
                    }
                    TransactionAction::Create => {
                        let scheme = CreateScheme::Legacy { caller: addr };
                        contract_addr = executor.create_address(scheme);
                        (
                            executor.transact_create(
                                addr, tx.value, tx.input, gas_limit, al,
                            ),
                            vec![],
                        )
                    }
                }
            }
            TransactionAny::EIP1559(tx) => {
                let gas_limit = tx.gas_limit.try_into().unwrap_or(u64::MAX);
                let al = tx
                    .access_list
                    .into_iter()
                    .map(|al| (al.address, al.slots))
                    .collect();
                match tx.action {
                    TransactionAction::Call(target) => {
                        contract_addr = target;
                        executor.transact_call(
                            addr, target, tx.value, tx.input, gas_limit, al,
                        )
                    }
                    TransactionAction::Create => {
                        let scheme = CreateScheme::Legacy { caller: addr };
                        contract_addr = executor.create_address(scheme);
                        (
                            executor.transact_create(
                                addr, tx.value, tx.input, gas_limit, al,
                            ),
                            vec![],
                        )
                    }
                }
            }
        };

        let gas_used = U256::from(executor.used_gas());
        let success = matches!(exit_reason, ExitReason::Succeed(_));
        let (changes, logs) = executor.into_state().deconstruct();
        if success {
            backend.apply(changes, logs.clone(), false);
        } else {
            backend.apply(
                Vec::<Apply<BTreeMap<H256, H256>>>::new(),
                logs.clone(),
                false,
            );
        }

        ExecRet {
            success,
            exit_reason,
            gas_used,
            fee_used: gas_used * gas_price,
            extra_data,
            caller: addr,
            contract_addr,
            logs,
        }
    }

    #[inline(always)]
    fn check_gas_price(&self, sb: &StateBranch, b: BranchName) -> Result<U256> {
        let gas_price_min = sb
            .state
            .evm
            .gas_price
            .get_value_by_branch(b)
            .unwrap_or(*GAS_PRICE_MIN);

        let gas_price = match &self.tx {
            TransactionAny::Legacy(tx) => tx.gas_price,
            TransactionAny::EIP2930(tx) => tx.gas_price,
            TransactionAny::EIP1559(tx) => tx.max_fee_per_gas,
        };

        if gas_price_min <= gas_price {
            Ok(gas_price)
        } else {
            Err(eg!("Gas price is too low"))
        }
    }

    fn check_balance(
        &self,
        addr: &H160,
        gas_price: U256,
        sb: &StateBranch,
        b: BranchName,
    ) -> StdResult<(OvrAccount, NeededAmount), Option<(OvrAccount, NeededAmount)>> {
        let (transfer_value, gas_limit) = match &self.tx {
            TransactionAny::Legacy(tx) => (tx.value, tx.gas_limit),
            TransactionAny::EIP2930(tx) => (tx.value, tx.gas_limit),
            TransactionAny::EIP1559(tx) => (tx.value, tx.gas_limit),
        };

        if gas_limit.is_zero() {
            return Err(None);
        }

        let needed_amount = gas_price
            .checked_mul(gas_limit)
            .and_then(|fee_limit| transfer_value.checked_add(fee_limit))
            .ok_or(None)?;

        let account = sb
            .state
            .evm
            .OFUEL
            .accounts
            .get_by_branch(addr, b)
            .unwrap_or_default();

        if needed_amount <= account.balance {
            Ok((account, needed_amount))
        } else {
            Err(Some((account, needed_amount)))
        }
    }

    #[inline(always)]
    fn check_nonce(
        &self,
        addr: &H160,
        sb: &StateBranch,
        b: BranchName,
    ) -> StdResult<(), (U256, U256)> {
        let tx_nonce = match &self.tx {
            TransactionAny::Legacy(tx) => tx.nonce,
            TransactionAny::EIP2930(tx) => tx.nonce,
            TransactionAny::EIP1559(tx) => tx.nonce,
        };

        let system_nonce = sb
            .state
            .evm
            .OFUEL
            .accounts
            .get_by_branch(addr, b)
            .map(|a| a.nonce)
            .unwrap_or_else(U256::zero);

        if tx_nonce == system_nonce {
            Ok(())
        } else {
            Err((tx_nonce, system_nonce))
        }
    }

    // if success, the transaction signature is valid.
    fn recover_signer(&self) -> Option<H160> {
        let pubkey = self.recover_pubkey()?;
        Some(H160::from(H256::from_slice(
            Keccak256::digest(&pubkey).as_slice(),
        )))
    }

    pub fn recover_pubkey(&self) -> Option<[u8; 64]> {
        let transaction = &self.tx;
        let mut sig = [0u8; 65];
        let mut msg = [0u8; 32];
        match transaction {
            TransactionAny::Legacy(t) => {
                sig[0..32].copy_from_slice(&t.signature.r()[..]);
                sig[32..64].copy_from_slice(&t.signature.s()[..]);
                sig[64] = t.signature.standard_v();
                msg.copy_from_slice(
                    &ethereum::LegacyTransactionMessage::from(t.clone()).hash()[..],
                );
            }
            TransactionAny::EIP2930(t) => {
                sig[0..32].copy_from_slice(&t.r[..]);
                sig[32..64].copy_from_slice(&t.s[..]);
                sig[64] = t.odd_y_parity as u8;
                msg.copy_from_slice(
                    &ethereum::EIP2930TransactionMessage::from(t.clone()).hash()[..],
                );
            }
            TransactionAny::EIP1559(t) => {
                sig[0..32].copy_from_slice(&t.r[..]);
                sig[32..64].copy_from_slice(&t.s[..]);
                sig[64] = t.odd_y_parity as u8;
                msg.copy_from_slice(
                    &ethereum::EIP1559TransactionMessage::from(t.clone()).hash()[..],
                );
            }
        }
        sp_io::crypto::secp256k1_ecdsa_recover(&sig, &msg).ok()
    }

    pub fn get_from_to(&self) -> (Option<H160>, Option<H160>) {
        let from = self.recover_signer();
        let to = match &self.tx {
            TransactionAny::Legacy(l) => match l.action {
                TransactionAction::Call(addr) => Some(addr),
                TransactionAction::Create => None,
            },
            TransactionAny::EIP2930(e) => match e.action {
                TransactionAction::Call(addr) => Some(addr),
                TransactionAction::Create => None,
            },
            TransactionAny::EIP1559(e) => match e.action {
                TransactionAction::Call(addr) => Some(addr),
                TransactionAction::Create => None,
            },
        };
        (from, to)
    }

    pub fn get_tx_common_properties(&self) -> TxCommonProperties {
        let (nonce, gas_limit, gas_price, input, value, action, r, s, v) = match &self.tx
        {
            TransactionAny::Legacy(tx) => (
                tx.nonce,
                tx.gas_limit,
                tx.gas_price,
                tx.input.clone(),
                tx.value,
                tx.action,
                *tx.signature.r(),
                *tx.signature.s(),
                tx.signature.standard_v(),
            ),
            TransactionAny::EIP2930(tx) => (
                tx.nonce,
                tx.gas_limit,
                tx.gas_price,
                tx.input.clone(),
                tx.value,
                tx.action,
                tx.r,
                tx.s,
                tx.odd_y_parity as u8,
            ),
            TransactionAny::EIP1559(tx) => {
                // Here I have taken a middle value
                // I don't think it should overflow here
                let price = tx
                    .max_priority_fee_per_gas
                    .saturating_add(tx.max_fee_per_gas)
                    .checked_div(U256::from(2))
                    .unwrap_or_default();

                (
                    tx.nonce,
                    tx.gas_limit,
                    price,
                    tx.input.clone(),
                    tx.value,
                    tx.action,
                    tx.r,
                    tx.s,
                    tx.odd_y_parity as u8,
                )
            }
        };

        TxCommonProperties {
            nonce,
            gas_limit,
            gas_price,
            action,
            value,
            input,
            r,
            s,
            v,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExecRet {
    pub success: bool,
    pub gas_used: U256,
    pub fee_used: U256,
    pub exit_reason: ExitReason,
    pub extra_data: Vec<u8>,
    pub caller: H160,
    pub contract_addr: H160,
    pub logs: Vec<Log>,
}

impl ExecRet {
    fn gen_receipt(&self, from: Option<H160>, to: Option<H160>) -> Receipt {
        let contract_addr = if to.is_none() {
            Some(self.contract_addr)
        } else {
            None
        };

        Receipt {
            tx_hash: vec![],
            tx_index: 0,
            from,
            to,
            block_gas_used: Default::default(),
            tx_gas_used: self.gas_used,
            contract_addr,
            state_root: None,
            status_code: self.success,
            logs: vec![],
        }
    }

    pub fn gen_logs<'a>(&'a self, tx_hash: HashValueRef<'a>) -> Vec<LedgerLog> {
        let mut v = Vec::new();
        for l in self.logs.iter() {
            v.push(LedgerLog::new_from_eth_log_and_tx_hash(l, tx_hash));
        }
        v
    }
}

impl fmt::Display for ExecRet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", serde_json::to_string(self).unwrap())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TxCommonProperties {
    pub nonce: U256,
    pub gas_limit: U256,
    pub gas_price: U256,
    pub action: TransactionAction,
    pub value: U256,
    pub input: Vec<u8>,
    pub r: H256,
    pub s: H256,
    pub v: u8,
}

pub fn inital_create2(
    contract: InitalContract,
    state: &super::State,
    b: BranchName<'_>,
) -> Result<()> {
    let evm_cfg = EvmCfg::istanbul();

    let metadata = StackSubstateMetadata::new(u64::MAX, &evm_cfg);
    let mut backend = state.get_backend_hdr(b);
    let state = OvrStackState::new(metadata, &backend);

    let precompiles = PRECOMPILE_SET.clone();
    let mut executor =
        StackExecutor::new_with_precompiles(state, &evm_cfg, &precompiles);

    let bytecode_hex = &contract.bytecode[2..].trim();

    // parse hex.
    let bytecode = hex::decode(bytecode_hex).c(d!())?;
    // get salt.
    let salt = H256::from_slice(&Keccak256::digest(contract.salt));

    let code_hash = H256::from_slice(&Keccak256::digest(&bytecode));

    let contract_addr = executor.create_address(CreateScheme::Create2 {
        caller: contract.from,
        salt,
        code_hash,
    });

    println!("OVR contract address is : {:?}", contract_addr);

    let exit_reason = executor.transact_create2(
        contract.from,
        U256::from(0u64),
        bytecode,
        salt,
        800000000,
        vec![],
    );

    let success = matches!(exit_reason, ExitReason::Succeed(_));
    if success {
        let (changes, logs) = executor.into_state().deconstruct();
        backend.apply(changes, logs, false);
    } else {
        return Err(eg!("inital create false."));
    }

    Ok(())
}
