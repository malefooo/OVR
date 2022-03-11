use jsonrpc_core::{BoxFuture, Result};
use web3_rpc_core::{types::PeerCount, NetApi};

pub struct NetApiImpl {}

impl NetApi for NetApiImpl {
    fn version(&self) -> BoxFuture<Result<String>> {
        let git_version = env!("VERGEN_GIT_SHA");
        Box::pin(async { Ok(git_version.to_string()) })
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
