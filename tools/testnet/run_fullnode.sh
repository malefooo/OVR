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

tm_home="${HOME}/.tendermint"
app_log_path="/tmp/testnet_app.log"
tendermint_log_path="/tmp/testnet_tendermint.log"

ovr daemon -d ${tm_home}/__vsdb__ >>${app_log_path} 2>&1 &
echo "**** ovr-daemon log path: ${app_log_path}"

tendermint node --home ${tm_home} >>${tendermint_log_path} 2>&1 &
echo "**** Tendermint log path: ${tendermint_log_path}"
