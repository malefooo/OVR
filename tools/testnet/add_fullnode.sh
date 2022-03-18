#!/usr/bin/env bash

#################################################
#### Ensure we are in the right path. ###########
#################################################
if [[ 0 -eq $(echo $0 | grep -c '^/') ]]; then
    # relative path
    EXEC_PATH=$(dirname "`pwd`/$0")
else
    # absolute path
    EXEC_PATH=$(dirname "$0")
fi

EXEC_PATH=$(echo ${EXEC_PATH} | sed 's@/\./@/@g' | sed 's@/\.*$@@')
cd $EXEC_PATH || exit 1
#################################################

env=$1

th="${HOME}/.tendermint"
tc="${th}/config/config.toml"
h="${th}/__ovr__"

check_env() {
    for i in tendermint curl perl; do
        which $i >/dev/null 2>&1
        if [[ 0 -ne $? ]]; then
            echo -e "\n\033[31;01m${i}\033[00m has not been installed properly!\n"
            exit 1
        fi
    done
}

serv_url=
sentry_peers=
set_env() {
    if [[ $env == "testnet" ]]; then
        serv_url="https://prod-testnet.prod.overeality.io"
        sentry_peers="1c8c2f7c4dde2db536b1cfc45526340667396555\@prod-testnet-us-west-2-sentry-000-open.prod.overeality.io:26656,115af0c43ce0e677ae48751da104a38d5876232e\@prod-testnet-us-west-2-sentry-001-open.prod.overeality.io:26656"
    elif [[ $env == "mainnet" ]]; then
        serv_url="https://prod-mainnet.prod.overeality.io"
        sentry_peers="1c8c2f7c4dde2db536b1cfc45526340667396555\@prod-mainnet-us-west-2-sentry-000-open.prod.overeality.io:26656,115af0c43ce0e677ae48751da104a38d5876232e\@prod-mainnet-us-west-2-sentry-001-open.prod.overeality.io:26656"
    else
        echo -e "Unknown ENV !!!"
        exit 1
    fi

    rm -rf $h $th
    mkdir -p $h $th
    tendermint init

    perl -pi -e "s/^(persistent_peers = ).*/\$1 \"${sentry_peers}\"/" $tc

    curl ${serv_url}:26657/genesis \
        | jq -c '.result.genesis' \
        | jq > ~/.tendermint/config/genesis.json || exit 1
}

if [[ "" == $2 ]]; then
    check_env
    set_env
fi

###################
# Run local node #
###################

bash ./run_fullnode.sh
