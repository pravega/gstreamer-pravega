#!/bin/bash
set -e

if [[ (-z "$1") || (-z "$2") ]]; then
    echo -e "Usage example:\n  $0 1.18.1 0"
    exit 1
fi

docker push pravega/gstreamer:$1-dev-with-source
docker tag pravega/gstreamer:$1-dev-with-source pravega/gstreamer:$1.$2-dev-with-source
docker push pravega/gstreamer:$1.$2-dev-with-source

docker push pravega/gstreamer:$1-dev
docker tag pravega/gstreamer:$1-dev pravega/gstreamer:$1.$2-dev
docker push pravega/gstreamer:$1.$2-dev

docker push pravega/gstreamer:$1-prod
docker tag pravega/gstreamer:$1-prod pravega/gstreamer:$1.$2-prod
docker push pravega/gstreamer:$1.$2-prod

docker push pravega/gstreamer:$1-prod-dbg
docker tag pravega/gstreamer:$1-prod-dbg pravega/gstreamer:$1.$2-prod-dbg
docker push pravega/gstreamer:$1.$2-prod-dbg
