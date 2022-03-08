![GitHub top language](https://img.shields.io/github/languages/top/ovr-defi/OVR)
![GitHub issues](https://img.shields.io/github/issues-raw/ovr-defi/OVR)
![GitHub pull requests](https://img.shields.io/github/issues-pr-raw/ovr-defi/OVR)
![GitHub Workflow Status](https://img.shields.io/github/workflow/status/ovr-defi/OVR/Rust)
![Minimum rustc version](https://img.shields.io/badge/rustc-1.59+-lightgray.svg)

# OVR

Overeality project.

## Usage

Quick start:

```shell
# compile binaries
make
# crate a local cluster
ovr dev --env-create --block-itv-secs 10
# stop it
ovr dev --env-stop
# start it again
ovr dev --env-start
# destroy it
ovr dev --env-destroy
```

Top-level overview:

```shell
ovr 0.3.x
OVR development team, fanhui.x@gmail.com
Official implementations of the Overeality project.

USAGE:
    ovr <SUBCOMMAND>

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information

SUBCOMMANDS:
    cli       Run ovr in client mode
    daemon    Run ovr in daemon mode, aka run a node
    dev       Development utils, create a local env, .etc
    help      Print this message or the help of the given subcommand(s)
```

A very useful sub-command for developers:

```shell
ovr-dev
Development utils, create a local env, .etc

USAGE:
    ovr dev [OPTIONS]

OPTIONS:
    -a, --env-add-node
    -c, --env-create
    -d, --env-destroy
    -h, --help                               Print help information
    -i, --env-info
    -I, --block-itv-secs <BLOCK_ITV_SECS>    [default: 1]
    -n, --env-name <ENV_NAME>
    -N, --validator-num <VALIDATOR_NUM>      How many validators should be created [default: 3]
    -r, --env-rm-node
    -s, --env-start
    -S, --env-stop
```

Generate a new account:

```shell
# ovr cli -g
Address: 0x0bab883d3adb7ec30f8b6ec9f3fc7265d20cae94
Phrase: face coach arrive affair gasp winner slow focus romance nothing project lesson
```
