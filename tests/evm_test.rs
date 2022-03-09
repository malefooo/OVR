#![allow(warnings)]

use ethereum::{
    AccessListItem, EIP1559Transaction, EIP2930Transaction, LegacyTransaction,
    TransactionAction, TransactionAny, TransactionSignature, TransactionV1,
    TransactionV2,
};
use evm::backend::ApplyBackend;
use evm::ExitReason;
use ovr::{
    ethvm::{
        impls::backend::OvrBackend,
        tx::{ExecRet, Tx},
        OvrAccount, OvrVicinity,
    },
    ledger::{Ledger, StateBranch},
    tx, EvmTx,
};
use primitive_types::{H160, H256, U256};
use sha3::{Digest, Keccak256};
use std::{io::Read, str::FromStr};
use vsdb::{BranchName, KeyEnDe};
const ADDR1: &str = "0xfa3805d34f4dc1da443a6b606feeb37374f472b1";
const ADDR2: &str = "0xd3265e2df8e4c67b1c496ac3970015db2c5b9d29";

fn init_state() -> Ledger {
    let addr1 = H160::from_str(ADDR1).unwrap();
    let acc1 = OvrAccount {
        nonce: U256::from(1u128),
        balance: U256::from(1000000000000000000000000u128),
        code: vec![],
    };

    let addr2 = H160::from_str(ADDR2).unwrap();
    let acc2 = OvrAccount {
        nonce: U256::from(10u128),
        balance: U256::from(1000000000000000000000000u128),
        code: vec![],
    };

    let mut ledger = Ledger::new(
        1234,
        String::from("TEST"),
        String::from("1"),
        Some(10000000000),
        Some(3000000),
        Some(1),
    )
    .unwrap();
    ledger
        .state
        .evm
        .OFUEL
        .accounts
        .insert_by_branch(addr1, acc1, BranchName(b"Main"));
    ledger
        .state
        .evm
        .OFUEL
        .accounts
        .insert_by_branch(addr2, acc2, BranchName(b"Main"));
    ledger
}

#[test]
fn test_evm_simple_transfer() {
    let ledger = init_state();
    let mut sb = StateBranch::new(&ledger.state, BranchName(b"Main")).unwrap();
    let addr1 = H160::from_str(ADDR1).unwrap();
    let addr2 = H160::from_str(ADDR2).unwrap();

    let r = H256([
        57, 149, 22, 170, 249, 82, 224, 123, 220, 61, 8, 93, 111, 212, 254, 10, 135, 74,
        92, 173, 250, 4, 154, 126, 109, 101, 218, 161, 180, 13, 177, 186,
    ]);
    let s = H256([
        63, 37, 146, 220, 140, 130, 87, 239, 223, 113, 55, 215, 173, 89, 242, 54, 186,
        139, 202, 101, 204, 108, 36, 4, 27, 144, 73, 85, 164, 251, 54, 242,
    ]);

    let tx = LegacyTransaction {
        nonce: U256::from(10u128),
        gas_price: U256::from(10000000000u128),
        gas_limit: U256::from(3000000u128),
        action: TransactionAction::Call(addr1),
        value: U256::from(1000000000u128),
        input: vec![],
        signature: TransactionSignature::new(2503, r, s).unwrap(),
    };

    let tx_v1 = TransactionV1::from(tx.clone());
    let evm_tx = EvmTx {
        tx: TransactionAny::from(tx_v1),
    };
    let t = tx::Tx::Evm(evm_tx.clone());
    let serialized = serde_json::to_vec(&t).unwrap();
    let raw_transfer_tx = hex::encode(serialized);
    println!("transfer tx:\n{:?}\n", raw_transfer_tx);

    let res = evm_tx.apply(&mut sb, BranchName(b"Main"), true);

    let acc1 = ledger
        .state
        .evm
        .OFUEL
        .accounts
        .get_by_branch(&addr1, BranchName(b"Main"))
        .unwrap();
    let acc2 = ledger
        .state
        .evm
        .OFUEL
        .accounts
        .get_by_branch(&addr2, BranchName(b"Main"))
        .unwrap();

    match res {
        Ok(ret) => {
            assert_eq!(
                U256::from_dec_str("1000000000000000000000000").unwrap()
                    + U256::from_dec_str("1000000000").unwrap(),
                acc1.balance,
                "acc1 balance error"
            );
            assert_eq!(
                U256::from_dec_str("1000000000000000000000000").unwrap()
                    - U256::from_dec_str("1000000000").unwrap(),
                acc2.balance,
                "acc2 balance error"
            );
            assert_eq!(true, ret.0.success, "execute tx failed");
            println!("acc1 balance: {:?}", acc1.balance);
            println!("acc2 balance: {:?}", acc2.balance);
        }
        _ => {
            assert!(false);
        }
    }
}

#[test]
fn test_evm_contract() {
    let ledger = init_state();
    let mut sb = StateBranch::new(&ledger.state, BranchName(b"Main")).unwrap();
    let addr1 = H160::from_str(ADDR1).unwrap();
    let addr2 = H160::from_str(ADDR2).unwrap();

    let r = H256([
        35, 177, 216, 146, 107, 73, 43, 56, 3, 10, 194, 122, 103, 113, 135, 82, 54, 205,
        213, 62, 67, 67, 156, 237, 36, 78, 95, 73, 110, 71, 124, 66,
    ]);
    let s = H256([
        10, 157, 209, 77, 53, 28, 144, 109, 76, 119, 17, 123, 174, 252, 19, 105, 229,
        250, 84, 138, 62, 205, 38, 226, 42, 16, 186, 69, 27, 74, 160, 26,
    ]);

    /// deploy contract
    /// read contract byte code
    let mut file = std::fs::File::open("./tests/contracts/token_bytecode.txt").unwrap();
    let mut content = String::new();
    let n = file.read_to_string(&mut content).unwrap();
    assert_ne!(0, n, "read error");
    let bytecode = hex::decode(content).unwrap();

    /// construct tx
    let mut nonce = 10u128;
    let tx = LegacyTransaction {
        nonce: U256::from(nonce),
        gas_price: U256::from(10000000000u128),
        gas_limit: U256::from(3000000u128),
        action: TransactionAction::Create,
        value: U256::zero(),
        input: bytecode,
        signature: TransactionSignature::new(2503, r, s).unwrap(),
    };
    let deploy_tx = TransactionV1::from(tx);
    let evm_deploy_tx = EvmTx {
        tx: TransactionAny::from(deploy_tx),
    };

    let t1 = tx::Tx::Evm(evm_deploy_tx.clone());
    let serialized1 = serde_json::to_vec(&t1).unwrap();
    let raw_deploy_tx = hex::encode(serialized1);
    println!("deploy tx:");
    println!("{:?}", raw_deploy_tx);
    println!();

    /// apply tx
    let mut contract: H160 = Default::default();
    let res = evm_deploy_tx.apply(&mut sb, BranchName(b"Main"), true);
    match res {
        Ok(ret) => {
            contract = ret.0.contract_addr;
            assert_eq!(
                true, ret.0.success,
                "deploy contract failed: {:?}",
                ret.0.exit_reason
            );
        }
        _ => {
            assert!(false);
        }
    }
    println!("contract:\n{:?}\n", contract);

    /// call contract
    let acc2 = ledger
        .state
        .evm
        .OFUEL
        .accounts
        .get_by_branch(&addr2, BranchName(b"Main"))
        .unwrap();
    assert_eq!(
        U256::from(11u128),
        acc2.nonce,
        "acc2 nonce is {}, expected {}",
        acc2.nonce,
        U256::from(11u128)
    );

    /// call balanceOf(address)
    let r = H256([
        87, 149, 159, 181, 146, 79, 154, 153, 124, 129, 157, 136, 228, 185, 175, 80,
        243, 83, 183, 155, 69, 99, 110, 214, 107, 23, 188, 42, 63, 153, 70, 204,
    ]);
    let s = H256([
        106, 65, 95, 167, 105, 98, 23, 137, 81, 167, 18, 70, 72, 80, 234, 216, 32, 51,
        42, 97, 50, 97, 136, 13, 249, 156, 47, 243, 126, 39, 125, 39,
    ]);
    let tx_input = hex::decode(
        "70a08231000000000000000000000000d3265e2df8e4c67b1c496ac3970015db2c5b9d29",
    )
    .unwrap();

    let balance_of = LegacyTransaction {
        nonce: U256::from(acc2.nonce),
        gas_price: U256::from(10000000000u128),
        gas_limit: U256::from(3000000u128),
        action: TransactionAction::Call(contract),
        value: U256::zero(),
        input: tx_input,
        signature: TransactionSignature::new(2504, r, s).unwrap(),
    };
    let tx_balance_of = TransactionV1::from(balance_of);
    let evm_tx_balance_of = EvmTx {
        tx: TransactionAny::from(tx_balance_of),
    };

    let t2 = tx::Tx::Evm(evm_tx_balance_of.clone());
    let serialized2 = serde_json::to_vec(&t2).unwrap();
    let balance_of_acc2_tx = hex::encode(serialized2);
    println!("balance of acc2 tx:\n{:?}\n", balance_of_acc2_tx);

    let res = evm_tx_balance_of.apply(&mut sb, BranchName(b"Main"), true);
    match res {
        Ok(ret) => {
            assert_eq!(
                true, ret.0.success,
                "get acc2 balance failed: {:?}",
                ret.0.exit_reason
            );
            let balance = U256::from_big_endian(ret.0.extra_data.as_slice());
            println!("after deploy, acc2 erc20 balance: {:?} \n", balance);
            let target_balance =
                U256::from_dec_str("1000000000000000000000000").unwrap();
            assert_eq!(
                balance, target_balance,
                "acc2 balance is {}, expected {}",
                balance, target_balance
            );
        }
        _ => {
            assert!(false);
        }
    }

    /// transfer: addr2 -> addr1: 90000000000000000000
    let acc2 = ledger
        .state
        .evm
        .OFUEL
        .accounts
        .get_by_branch(&addr2, BranchName(b"Main"))
        .unwrap();
    assert_eq!(
        U256::from(12u128),
        acc2.nonce,
        "acc2 nonce is {}, expected {}",
        acc2.nonce,
        U256::from(12u128)
    );
    let r = H256([
        5, 58, 127, 17, 80, 110, 168, 12, 225, 184, 93, 241, 9, 216, 55, 123, 40, 164,
        10, 124, 143, 173, 251, 62, 109, 42, 195, 225, 118, 129, 169, 230,
    ]);
    let s = H256([
        105, 253, 34, 31, 186, 255, 63, 114, 28, 157, 56, 139, 250, 166, 22, 86, 187,
        161, 10, 182, 126, 183, 240, 183, 12, 41, 77, 180, 98, 140, 128, 85,
    ]);
    let tx_input = hex::decode(
        "a9059cbb000000000000000000000000fa3805d34f4dc1da443a6b606feeb37374f472b1000000000000000000000000000000000000000000000004e1003b28d9280000"
    ).unwrap();

    let transfer = LegacyTransaction {
        nonce: U256::from(acc2.nonce),
        gas_price: U256::from(10000000000u128),
        gas_limit: U256::from(3000000u128),
        action: TransactionAction::Call(contract),
        value: U256::zero(),
        input: tx_input,
        signature: TransactionSignature::new(2504, r, s).unwrap(),
    };
    let tx_transfer = TransactionV1::from(transfer);
    let evm_tx_transfer = EvmTx {
        tx: TransactionAny::from(tx_transfer),
    };

    let t3 = tx::Tx::Evm(evm_tx_transfer.clone());
    let serialized3 = serde_json::to_vec(&t3).unwrap();
    let transfer_tx = hex::encode(serialized3);
    println!("transfer from acc2 to acc1 tx:\n{:?}\n", transfer_tx);

    let res = evm_tx_transfer.apply(&mut sb, BranchName(b"Main"), true);
    match res {
        Ok(ret) => {
            assert_eq!(
                true, ret.0.success,
                "transfer failed: {:?}",
                ret.0.exit_reason
            );
        }
        _ => {
            assert!(false);
        }
    }

    /// get acc1 balance
    let acc1 = ledger
        .state
        .evm
        .OFUEL
        .accounts
        .get_by_branch(&addr1, BranchName(b"Main"))
        .unwrap();
    assert_eq!(
        U256::from(1u128),
        acc1.nonce,
        "acc2 nonce is {}, expected {}",
        acc1.nonce,
        U256::from(1u128)
    );
    let r = H256([
        135, 43, 179, 76, 185, 42, 50, 181, 254, 2, 146, 118, 222, 19, 26, 82, 58, 232,
        175, 159, 112, 59, 65, 127, 136, 238, 6, 168, 183, 93, 178, 214,
    ]);
    let s = H256([
        52, 18, 98, 128, 94, 118, 32, 234, 224, 31, 148, 149, 168, 231, 226, 137, 175,
        204, 48, 189, 58, 41, 51, 250, 63, 40, 127, 35, 0, 35, 199, 94,
    ]);
    let tx_input_acc1 = hex::decode(
        "70a08231000000000000000000000000fa3805d34f4dc1da443a6b606feeb37374f472b1",
    )
    .unwrap();
    let balance_of_acc1 = LegacyTransaction {
        nonce: U256::from(acc1.nonce),
        gas_price: U256::from(10000000000u128),
        gas_limit: U256::from(3000000u128),
        action: TransactionAction::Call(contract),
        value: U256::zero(),
        input: tx_input_acc1,
        signature: TransactionSignature::new(2503, r, s).unwrap(),
    };
    let tx_balance_of_acc1 = TransactionV1::from(balance_of_acc1);
    let evm_tx_balance_of_acc1 = EvmTx {
        tx: TransactionAny::from(tx_balance_of_acc1),
    };

    let t4 = tx::Tx::Evm(evm_tx_balance_of_acc1.clone());
    let serialized4 = serde_json::to_vec(&t4).unwrap();
    let balance_of_acc1_erc20_tx = hex::encode(serialized4);
    println!(
        "\nbalance of acc1 erc20 tx:{:?}\n",
        balance_of_acc1_erc20_tx
    );

    let res = evm_tx_balance_of_acc1.apply(&mut sb, BranchName(b"Main"), true);
    let mut acc1_erc20_balance = U256::default();
    match res {
        Ok(ret) => {
            assert_eq!(
                true, ret.0.success,
                "get acc1 erc20 balance failed: {:?}",
                ret.0.exit_reason
            );
            acc1_erc20_balance = U256::from_big_endian(ret.0.extra_data.as_slice());
        }
        _ => {
            assert!(false);
        }
    }
    println!(
        "after transfer, acc1 erc20 balance: {:?}\n",
        acc1_erc20_balance
    );
    assert_eq!(
        acc1_erc20_balance,
        U256::from_dec_str("90000000000000000000").unwrap(),
        "acc1 erc20 balance is {}, expected {}",
        acc1_erc20_balance,
        U256::from_dec_str("90000000000000000000").unwrap()
    );

    /// get acc2 balance
    let acc2 = ledger
        .state
        .evm
        .OFUEL
        .accounts
        .get_by_branch(&addr2, BranchName(b"Main"))
        .unwrap();
    assert_eq!(
        U256::from(13u128),
        acc2.nonce,
        "acc2 nonce is {}, expected {}",
        acc2.nonce,
        U256::from(13u128)
    );

    let r = H256([
        246, 94, 67, 117, 200, 121, 242, 142, 92, 116, 58, 157, 104, 59, 167, 148, 94,
        74, 197, 37, 198, 159, 97, 104, 113, 195, 20, 246, 82, 138, 122, 173,
    ]);
    let s = H256([
        53, 219, 114, 136, 205, 121, 14, 66, 87, 60, 53, 57, 203, 71, 56, 226, 225, 198,
        107, 190, 190, 154, 187, 116, 174, 218, 210, 18, 3, 79, 244, 105,
    ]);
    let tx_input_acc1 = hex::decode(
        "70a08231000000000000000000000000d3265e2df8e4c67b1c496ac3970015db2c5b9d29",
    )
    .unwrap();
    let balance_of_acc2 = LegacyTransaction {
        nonce: U256::from(acc2.nonce),
        gas_price: U256::from(10000000000u128),
        gas_limit: U256::from(3000000u128),
        action: TransactionAction::Call(contract),
        value: U256::zero(),
        input: tx_input_acc1,
        signature: TransactionSignature::new(2504, r, s).unwrap(),
    };
    let tx_balance_of_acc2 = TransactionV1::from(balance_of_acc2);
    let evm_tx_balance_of_acc2 = EvmTx {
        tx: TransactionAny::from(tx_balance_of_acc2),
    };

    let t5 = tx::Tx::Evm(evm_tx_balance_of_acc2.clone());
    let serialized5 = serde_json::to_vec(&t5).unwrap();
    let balance_of_acc2_erc20_tx = hex::encode(serialized5);
    println!(
        "balance of acc1 erc20 tx:\n{:?}\n",
        balance_of_acc2_erc20_tx
    );

    let res = evm_tx_balance_of_acc2.apply(&mut sb, BranchName(b"Main"), true);
    let mut acc2_erc20_balance = U256::default();
    match res {
        Ok(ret) => {
            assert_eq!(
                true, ret.0.success,
                "get acc2 erc20 balance failed: {:?}",
                ret.0.exit_reason
            );
            acc2_erc20_balance = U256::from_big_endian(ret.0.extra_data.as_slice());
        }
        _ => {
            assert!(false);
        }
    }
    println!(
        "after transfer, acc2 erc20 balance: {:?}\n",
        acc2_erc20_balance
    );
    let acc2_expected_balance = U256::from_dec_str("1000000000000000000000000").unwrap()
        - U256::from_dec_str("90000000000000000000").unwrap();
    assert_eq!(
        acc2_erc20_balance, acc2_expected_balance,
        "acc2 erc20 balance is {}, expected {}",
        acc2_erc20_balance, acc2_expected_balance
    );
}

#[test]
fn test_legacy_tx() {
    let ledger = init_state();
    let mut sb = StateBranch::new(&ledger.state, BranchName(b"Main")).unwrap();
    let addr1 = H160::from_str(ADDR1).unwrap();
    let addr2 = H160::from_str(ADDR2).unwrap();

    let r = H256([
        57, 149, 22, 170, 249, 82, 224, 123, 220, 61, 8, 93, 111, 212, 254, 10, 135, 74,
        92, 173, 250, 4, 154, 126, 109, 101, 218, 161, 180, 13, 177, 186,
    ]);
    let s = H256([
        63, 37, 146, 220, 140, 130, 87, 239, 223, 113, 55, 215, 173, 89, 242, 54, 186,
        139, 202, 101, 204, 108, 36, 4, 27, 144, 73, 85, 164, 251, 54, 242,
    ]);

    let tx = LegacyTransaction {
        nonce: U256::from(10u128),
        gas_price: U256::from(10000000000u128),
        gas_limit: U256::from(3000000u128),
        action: TransactionAction::Call(addr1),
        value: U256::from(1000000000u128),
        input: vec![],
        signature: TransactionSignature::new(2503, r, s).unwrap(),
    };

    let tx_v1 = TransactionV1::from(tx.clone());
    let evm_tx = EvmTx {
        tx: TransactionAny::from(tx_v1),
    };
    let t = tx::Tx::Evm(evm_tx.clone());
    let serialized = serde_json::to_vec(&t).unwrap();
    let raw_transfer_tx = hex::encode(serialized);
    println!("transfer tx:\n{:?}\n", raw_transfer_tx);

    let _ = sb.apply_tx(t);

    let acc1 = ledger
        .state
        .evm
        .OFUEL
        .accounts
        .get_by_branch(&addr1, BranchName(b"Main"))
        .unwrap();
    let acc2 = ledger
        .state
        .evm
        .OFUEL
        .accounts
        .get_by_branch(&addr2, BranchName(b"Main"))
        .unwrap();

    assert_eq!(
        U256::from_dec_str("1000000000000000000000000").unwrap()
            + U256::from_dec_str("1000000000").unwrap(),
        acc1.balance,
        "acc1 balance error"
    );
    println!("acc1 balance: {:?}\n", acc1.balance);
    println!("acc2 balance: {:?}\n", acc2.balance);

    let acc2_balance_without_tx_fee = U256::from_dec_str("1000000000000000000000000")
        .unwrap()
        - U256::from_dec_str("1000000000").unwrap();
    assert!(
        acc2.balance < acc2_balance_without_tx_fee,
        "the tx fee of acc2 is not deducted"
    );
    println!("tx fee: {:?}\n", acc2_balance_without_tx_fee - acc2.balance);
}

#[test]
fn test_eip2930_tx() {
    let ledger = init_state();
    let mut sb = StateBranch::new(&ledger.state, BranchName(b"Main")).unwrap();
    let addr1 = H160::from_str(ADDR1).unwrap();
    let addr2 = H160::from_str(ADDR2).unwrap();

    let tx = TransactionV1::EIP2930(EIP2930Transaction {
        chain_id: 1234,
        nonce: 10.into(),
        gas_price: 30_000_000_000_u64.into(),
        gas_limit: 6_000_000_u64.into(),
        action: TransactionAction::Call(addr1),
        value: U256::from(1) * 1_000_000_000,
        input: vec![],
        access_list: vec![AccessListItem {
            address: H160::from_str("fa3805d34f4dc1da443a6b606feeb37374f472b1").unwrap(),
            slots: vec![
                H256::from_str(
                    "0000000000000000000000000000000000000000000000000000000000000003",
                )
                .unwrap(),
                H256::from_str(
                    "0000000000000000000000000000000000000000000000000000000000000007",
                )
                .unwrap(),
            ],
        }],
        odd_y_parity: false,
        r: H256::from_str(
            "808fc214163e17777b8ce96729f85440caaaf965259e9a996766661bc33550f4",
        )
        .unwrap(),
        s: H256::from_str(
            "6fc97fe2819d5f5d67e615651d06319d520719eee93b726179231d0a3e8b78fd",
        )
        .unwrap(),
    });

    let evm_tx = EvmTx {
        tx: TransactionAny::from(tx),
    };
    let t = tx::Tx::Evm(evm_tx.clone());
    let serialized = serde_json::to_vec(&t).unwrap();
    let raw_transfer_tx = hex::encode(serialized);
    println!("transfer tx:\n{:?}\n", raw_transfer_tx);

    let _ = sb.apply_tx(t);

    let acc1 = ledger
        .state
        .evm
        .OFUEL
        .accounts
        .get_by_branch(&addr1, BranchName(b"Main"))
        .unwrap();
    let acc2 = ledger
        .state
        .evm
        .OFUEL
        .accounts
        .get_by_branch(&addr2, BranchName(b"Main"))
        .unwrap();

    assert_eq!(
        U256::from_dec_str("1000000000000000000000000").unwrap()
            + U256::from_dec_str("1000000000").unwrap(),
        acc1.balance,
        "acc1 balance error"
    );
    println!("acc1 balance: {:?}\n", acc1.balance);
    println!("acc2 balance: {:?}\n", acc2.balance);

    let acc2_balance_without_tx_fee = U256::from_dec_str("1000000000000000000000000")
        .unwrap()
        - U256::from_dec_str("1000000000").unwrap();
    assert!(
        acc2.balance < acc2_balance_without_tx_fee,
        "the tx fee of acc2 is not deducted"
    );
    println!("tx fee: {:?}\n", acc2_balance_without_tx_fee - acc2.balance);
}

#[test]
fn test_eip1559_tx() {
    let ledger = init_state();
    let mut sb = StateBranch::new(&ledger.state, BranchName(b"Main")).unwrap();
    let addr1 = H160::from_str(ADDR1).unwrap();
    let addr2 = H160::from_str(ADDR2).unwrap();
    let tx = TransactionV2::EIP1559(EIP1559Transaction {
        chain_id: 1234,
        nonce: 10.into(),
        max_priority_fee_per_gas: 10_000_000_000_u64.into(),
        max_fee_per_gas: 30_000_000_000_u64.into(),
        gas_limit: 6_000_000_u64.into(),
        action: TransactionAction::Call(addr1),
        value: U256::from(1) * 1_000_000_000,
        input: vec![],
        access_list: vec![AccessListItem {
            address: H160::from_str("fa3805d34f4dc1da443a6b606feeb37374f472b1").unwrap(),
            slots: vec![
                H256::from_str(
                    "0000000000000000000000000000000000000000000000000000000000000003",
                )
                .unwrap(),
                H256::from_str(
                    "0000000000000000000000000000000000000000000000000000000000000007",
                )
                .unwrap(),
            ],
        }],
        odd_y_parity: false,
        r: H256::from_str(
            "5e25a18b7d0722c79abfd95f2990b26457458d6c0b732db8fcfa4ababd60611e",
        )
        .unwrap(),
        s: H256::from_str(
            "62c93cfc833a8d9885b6840c234ccaea11fdf98f5b0a9f558c5e57b0bd6046ac",
        )
        .unwrap(),
    });

    let evm_tx = EvmTx {
        tx: TransactionAny::from(tx),
    };
    let t = tx::Tx::Evm(evm_tx.clone());
    let serialized = serde_json::to_vec(&t).unwrap();
    let raw_transfer_tx = hex::encode(serialized);
    println!("transfer tx:\n{:?}\n", raw_transfer_tx);

    let _ = sb.apply_tx(t);

    let acc1 = ledger
        .state
        .evm
        .OFUEL
        .accounts
        .get_by_branch(&addr1, BranchName(b"Main"))
        .unwrap();
    let acc2 = ledger
        .state
        .evm
        .OFUEL
        .accounts
        .get_by_branch(&addr2, BranchName(b"Main"))
        .unwrap();

    assert_eq!(
        U256::from_dec_str("1000000000000000000000000").unwrap()
            + U256::from_dec_str("1000000000").unwrap(),
        acc1.balance,
        "acc1 balance error"
    );
    println!("acc1 balance: {:?}\n", acc1.balance);
    println!("acc2 balance: {:?}\n", acc2.balance);

    let acc2_balance_without_tx_fee = U256::from_dec_str("1000000000000000000000000")
        .unwrap()
        - U256::from_dec_str("1000000000").unwrap();
    assert!(
        acc2.balance < acc2_balance_without_tx_fee,
        "the tx fee of acc2 is not deducted"
    );
    println!("tx fee: {:?}\n", acc2_balance_without_tx_fee - acc2.balance);
}
