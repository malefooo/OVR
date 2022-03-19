use clap::{Parser, Subcommand};
use ruc::*;

#[cfg(target_os = "linux")]
use {
    crate::common::BlockHeight,
    btm::{BtmCfg, SnapAlgo, SnapMode, ENV_VAR_BTM_VOLUME},
    std::env,
};

#[derive(Parser, Debug)]
#[clap(about, version, author)]
pub struct Cfg {
    #[clap(subcommand)]
    pub commands: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[clap(about = "Run ovr in client mode")]
    Cli(CliCfg),
    #[clap(about = "Run ovr in daemon mode, aka run a node")]
    Daemon(Box<DaemonCfg>),
    #[clap(about = "Development utils, creating a local env, etc.")]
    Dev(DevCfg),
    #[cfg(target_os = "linux")]
    #[clap(about = "BTM related operations")]
    Snap(SnapCfg),
}

#[derive(Clone, Debug, Parser)]
pub struct DaemonCfg {
    #[clap(
        long,
        default_value_t = 9527,
        help = "The ID of your chain, an unsigned integer"
    )]
    pub chain_id: u64,
    #[clap(
        long,
        default_value_t = String::from("NULL"),
        help = "A custom name of your chain"
    )]
    pub chain_name: String,
    #[clap(
        long,
        default_value_t = String::from("NULL"),
        help = "A custom version of your chain"
    )]
    pub chain_version: String,
    #[clap(long, help = "Basic gas price of the evm transactions")]
    pub gas_price: Option<u128>,
    #[clap(long, help = "The limitation of the total gas of any block")]
    pub block_gas_limit: Option<u128>,
    #[clap(
        short = 'd',
        long,
        help = "A path where all data will be stored in [default: ~/.vsdb]"
    )]
    pub vsdb_base_dir: Option<String>,
    #[clap(
        short = 'D',
        long,
        default_value_t = 2000,
        help = "How many versions shoule be always keeped"
    )]
    pub vsdb_prune_cap: usize,
    #[clap(
        short = 'H',
        long,
        help = "A path where tendermint will run in [default: ~/.tendermint]"
    )]
    pub tendermint_home_dir: Option<String>,
    #[clap(long, help = "A field for EIP1559")]
    pub block_base_fee_per_gas: Option<u128>,

    #[clap(
        short = 'A',
        long,
        default_value_t = ["0.0.0.0"].join(","),
        help = "Addresses served by the daemon, seperated by ','"
    )]
    pub serv_addr_list: String,
    #[clap(
        short = 'p',
        long,
        default_value_t = 8545,
        help = "Http service, a value of zero means disable the service"
    )]
    pub serv_http_port: u16,
    #[clap(
        short = 'w',
        long,
        default_value_t = 8546,
        help = "Websocket service, a value of zero means disable the service"
    )]
    pub serv_ws_port: u16,
    #[clap(
        short = 'a',
        long,
        default_value_t = 26658,
        help = "the listening port of tendermint ABCI process(embed in ovr)"
    )]
    pub serv_abci_port: u16,
    #[clap(
        short = 'T',
        long,
        default_value_t = 26657,
        help = "the listening port of tendermint RPC(embed in tendermint)"
    )]
    pub tendermint_rpc_port: u16,

    #[cfg(target_os = "linux")]
    #[clap(long, help = "Global switch of snapshot functions")]
    pub snap_enable: bool,

    #[cfg(target_os = "linux")]
    #[clap(
        short = 'P',
        long,
        help = "Will try to use ${ENV_VAR_BTM_VOLUME} if missing"
    )]
    pub snap_volume: Option<String>,

    #[cfg(target_os = "linux")]
    #[clap(
        short = 'M',
        long,
        help = "Will try to detect the local system if missing"
    )]
    pub snap_mode: Option<SnapMode>,

    #[cfg(target_os = "linux")]
    #[clap(long, default_value_t = SnapAlgo::Fair)]
    pub snap_algo: SnapAlgo,

    #[cfg(target_os = "linux")]
    #[clap(short = 'I', long, default_value_t = 10)]
    pub snap_itv: u64,

    #[cfg(target_os = "linux")]
    #[clap(short = 'C', long, default_value_t = 100)]
    pub snap_cap: u64,
}

impl DaemonCfg {
    #[inline(always)]
    #[cfg(target_os = "linux")]
    pub fn snapshot(&self, height: BlockHeight) -> Result<()> {
        BtmCfg::try_from(self).c(d!())?.snapshot(height).c(d!())
    }

    #[inline(always)]
    pub fn set_vsdb_base_dir(&self) -> Result<()> {
        if let Some(dir) = self.vsdb_base_dir.clone() {
            vsdb::vsdb_set_base_dir(dir).c(d!())?;
        }
        Ok(())
    }
}

#[derive(Debug, Parser)]
pub struct CliCfg {
    #[clap(
        short = 'A',
        long,
        default_value_t = String::from("localhost"),
        help = "Addresses served by the server end"
    )]
    pub serv_addr: String,
    #[clap(
        short = 'p',
        long,
        default_value_t = 8545,
        help = "A port used for http service"
    )]
    pub serv_http_port: u16,
    #[clap(
        short = 'w',
        long,
        default_value_t = 8546,
        help = "A port used for websocket service"
    )]
    pub serv_ws_port: u16,
    #[clap(short = 'g', long, help = "Generate a new account")]
    pub gen_account: bool,
}

#[derive(Debug, Parser)]
pub struct DevCfg {
    #[clap(short = 'n', long)]
    pub env_name: Option<String>,
    #[clap(short = 'c', long)]
    pub env_create: bool,
    #[clap(short = 'd', long)]
    pub env_destroy: bool,
    #[clap(short = 's', long)]
    pub env_start: bool,
    #[clap(short = 'S', long)]
    pub env_stop: bool,
    #[clap(short = 'a', long)]
    pub env_add_node: bool,
    #[clap(short = 'r', long)]
    pub env_rm_node: bool,
    #[clap(short = 'i', long)]
    pub env_info: bool,
    #[clap(short = 'I', long, default_value_t = 1)]
    pub block_itv_secs: u8,
    #[clap(
        short = 'N',
        long,
        default_value_t = 3,
        help = "How many validators should be created"
    )]
    pub validator_num: u8,
    #[clap(long)]
    pub inital_salt: Option<String>,
    #[clap(long)]
    pub inital_bytecode_path: Option<String>,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Parser)]
pub struct SnapCfg {
    #[clap(subcommand)]
    pub commands: SnapOps,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Subcommand)]
pub enum SnapOps {
    #[clap(about = "Rollback to a custom historical snapshot")]
    Rollback(SnapRollbackArgs),
    #[clap(about = "Clean up all existing snapshots")]
    Clean(SnapCleanArgs),
    #[clap(about = "List all existing snapshots")]
    List(SnapListArgs),
}

#[cfg(target_os = "linux")]
#[derive(Parser, Debug)]
pub struct SnapRollbackArgs {
    #[clap(
        short = 'P',
        long,
        help = "Will try to use ${ENV_VAR_BTM_VOLUME} if missing"
    )]
    pub volume: Option<String>,

    #[clap(
        short = 'H',
        long,
        help = "Will try to use the latest existing height if missing"
    )]
    pub height: Option<u64>,

    #[clap(
        short = 'X',
        long,
        help = "If specified, a snapshot must exist at the 'height'"
    )]
    pub exact: bool,

    #[clap(
        short = 'M',
        long,
        help = "Will try to detect the local system if missing"
    )]
    pub mode: Option<SnapMode>,
}

#[cfg(target_os = "linux")]
#[derive(Parser, Debug)]
pub struct SnapCleanArgs {
    #[clap(
        short = 'P',
        long,
        help = "Will try to use ${ENV_VAR_BTM_VOLUME} if missing"
    )]
    pub volume: Option<String>,

    #[clap(
        short = 'M',
        long,
        help = "Will try to detect the local system if missing"
    )]
    pub mode: Option<SnapMode>,
}

#[cfg(target_os = "linux")]
type SnapListArgs = SnapCleanArgs;

#[cfg(target_os = "linux")]
impl TryFrom<&DaemonCfg> for BtmCfg {
    type Error = Box<dyn RucError>;

    fn try_from(dc: &DaemonCfg) -> Result<Self> {
        let volume = dc
            .snap_volume
            .clone()
            .c(d!())
            .or_else(|_| env::var(ENV_VAR_BTM_VOLUME).c(d!()))?;
        let mode = dc
            .snap_mode
            .c(d!())
            .or_else(|e| Self::guess_mode(&volume).c(d!(e)))?;
        Ok(Self {
            enable: dc.snap_enable,
            itv: dc.snap_itv,
            cap: dc.snap_cap,
            mode,
            algo: dc.snap_algo,
            volume,
        })
    }
}

#[cfg(target_os = "linux")]
impl TryFrom<&SnapCfg> for BtmCfg {
    type Error = Box<dyn RucError>;

    fn try_from(sc: &SnapCfg) -> Result<Self> {
        macro_rules! parse_args {
            ($args: expr) => {{
                let volume = $args
                    .volume
                    .clone()
                    .c(d!())
                    .or_else(|_| env::var(ENV_VAR_BTM_VOLUME).c(d!()))?;
                let mode = $args
                    .mode
                    .c(d!())
                    .or_else(|e| Self::guess_mode(&volume).c(d!(e)))?;
                (volume, mode)
            }};
        }

        let (volume, mode) = match &sc.commands {
            SnapOps::List(args) => {
                parse_args!(args)
            }
            SnapOps::Clean(args) => {
                parse_args!(args)
            }
            SnapOps::Rollback(args) => {
                parse_args!(args)
            }
        };

        Ok(Self {
            enable: true,
            mode,
            volume,
            ..Default::default()
        })
    }
}
