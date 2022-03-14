use crate::rpc::error::new_jsonrpc_error;
use jsonrpc_core::{BoxFuture, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use web3_rpc_core::{types::PeerCount, NetApi};

pub struct NetApiImpl {}

#[derive(Deserialize, Serialize)]
struct VersionInfo<'a> {
    git_commit: &'a str,
    git_semver: &'a str,
    rustc_commit: &'a str,
    rustc_semver: &'a str,
}

impl NetApi for NetApiImpl {
    fn version(&self) -> BoxFuture<Result<Value>> {
        let vi = VersionInfo {
            git_commit: env!("VERGEN_GIT_SHA"),
            git_semver: env!("VERGEN_GIT_SEMVER"),
            rustc_commit: env!("VERGEN_RUSTC_COMMIT_HASH"),
            rustc_semver: env!("VERGEN_RUSTC_SEMVER"),
        };

        Box::pin(async move {
            match serde_json::to_value(&vi) {
                Ok(json) => Ok(json),
                Err(e) => Err(new_jsonrpc_error(e.to_string().as_str(), Value::Null)),
            }
        })
    }

    fn peer_count(&self) -> BoxFuture<Result<PeerCount>> {
        // try to get infomation from tendermint.
        Box::pin(async move { Ok(PeerCount::U32(0)) })
    }

    fn is_listening(&self) -> BoxFuture<Result<bool>> {
        // try to get infomation from tendermint.
        Box::pin(async move { Ok(true) })
    }
}
