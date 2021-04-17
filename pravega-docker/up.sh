#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0))
cd ${ROOT_DIR}
export HOST_IP=${HOST_IP:-192.168.120.128}
export PRAVEGA_LTS_PATH=${PRAVEGA_LTS_PATH:-/tmp/pravega-lts}
docker-compose down
sudo rm -rf ${PRAVEGA_LTS_PATH}
docker-compose up -d
sleep 10s
curl -X POST -H "Content-Type: application/json" -d '{"scopeName":"examples"}' http://localhost:10080/v1/scopes
docker-compose logs --follow
