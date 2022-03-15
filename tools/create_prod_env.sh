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

os=$(uname -s)

if [[ "Linux" == $os ]]; then
    make -C .. build_release_musl >/dev/null
else
    make -C .. build_release >/dev/null
fi

env_name="prodenv-${RANDOM}${RANDOM}"
if [[ "" != $1 ]]; then env_name=$1; fi
env_path="/tmp/__OVR_DEV__/${env_name}"
c_path="${EXEC_PATH}/submodules/system_contracts"
c_results_path="/tmp/results.tmp"
cmd="$(dirname ${EXEC_PATH})/release/ovr dev"

cd $c_path || exit 1
npm install \
    || (echo -e "\033[31;1mNeed to install 'node'?\033[0m" && exit 1)
npx hardhat compile \
    || (echo -e "\033[31;1mNeed to install 'node'?\033[0m" && exit 1)
cd $EXEC_PATH || exit 1

# build abi and bytecode
mkdir -p $c_results_path
cat ${c_path}/artifacts/contracts/ORToken.sol/ORToken.json \
    | jq .abi > ${c_results_path}/OR.abi.json || exit 1
cat ${c_path}/artifacts/contracts/ORToken.sol/ORToken.json \
    | jq -r .bytecode > ${c_results_path}/OR.bytecode || exit 1

echo $env_name
$cmd -d -n $env_name 2>/dev/null
$cmd -c -n $env_name -N 7 \
    --inital-bytecode-path ${c_results_path}/OR.bytecode \
    --inital-salt "ORToken" >/dev/null || exit 1
sleep 30
$cmd -S -n $env_name >/dev/null || exit 1

for cfg in $(find ${env_path} -name "config.toml"); do
    if [[ "" == $1 ]]; then
        perl -pi -e 's/^\s*(addr_book_strict)\s*=\s*.*/$1 = true/' $cfg
    fi
    perl -pi -e 's/^\s*(persistent_peers_max_dial_period)\s*=\s*.*/$1 = "3s"/' $cfg
    perl -pi -e 's/^\s*(timeout_propose)\s*=\s*.*/$1 = "3s"/' $cfg
    perl -pi -e 's/^\s*(timeout_propose_delta)\s*=\s*.*/$1 = "500ms"/' $cfg
    perl -pi -e 's/^\s*(timeout_prevote)\s*=\s*.*/$1 = "1s"/' $cfg
    perl -pi -e 's/^\s*(timeout_prevote_delta)\s*=\s*.*/$1 = "500ms"/' $cfg
    perl -pi -e 's/^\s*(timeout_precommit)\s*=\s*.*/$1 = "1s"/' $cfg
    perl -pi -e 's/^\s*(timeout_precommit_delta)\s*=\s*.*/$1 = "500ms"/' $cfg
    perl -pi -e 's/^\s*(timeout_commit)\s*=\s*.*/$1 = "1s"/' $cfg
    perl -pi -e 's/^\s*(create_empty_blocks)\s*=\s*.*/$1 = true/' $cfg
    perl -pi -e 's/^\s*(create_empty_blocks_interval)\s*=\s*.*/$1 = "0s"/' $cfg
done

pkg_dir="/tmp/prodenv"
pkg_name="prodenv.tar.gz"

rm -rf $pkg_dir $pkg_name 2>/dev/null
mkdir $pkg_dir || exit 1

for i in 9 8 7 6 5 4 3; do
    cp -r ${env_path}/${i} ${pkg_dir}/validator-$[i - 2] || exit 1
done
cp -r ${env_path}/2 ${pkg_dir}/full-1 || exit 1
cp -r ${env_path}/1 ${pkg_dir}/seed-1 || exit 1
cp ${env_path}/config.json ${pkg_dir}/env_config.json || exit 1

if [[ "" == $1 ]]; then
    rm -rf ${env_path} || exit 1
fi

tar -C $pkg_dir -zcpf $pkg_name . || exit 1
mv $pkg_name .. || exit 1

echo
echo -e "\033[1mPackage: \033[31;1m${pkg_name}\033[0m"
echo
