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
tm_mark="tendermint node --home ${tm_home}"
ovr_mark="ovr daemon -d ${tm_home}/__vsdb__"

for mark in "${tm_home}" "${ovr_mark}"; do
	for pid in $(ps ax -o pid,args \
		| grep "${mark}" \
		| grep -v 'grep' \
		| grep -Eo '^ *[0-9]+' \
		| sed 's/ //g'); do
        kill -9 $pid
	done
done
