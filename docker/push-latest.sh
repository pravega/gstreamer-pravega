#!/bin/bash
set -e

DATE=$(date -u +"%Y-%m-%dT%H-%M-%SZ")

echo $DATE

docker push pravega/gstreamer:latest-dev-with-source
docker tag pravega/gstreamer:latest-dev-with-source pravega/gstreamer:$DATE-dev-with-source
docker push pravega/gstreamer:$DATE-dev-with-source

docker push pravega/gstreamer:latest-dev
docker tag pravega/gstreamer:latest-dev pravega/gstreamer:$DATE-dev
docker push pravega/gstreamer:$DATE-dev

docker push pravega/gstreamer:latest-prod
docker tag pravega/gstreamer:latest-prod pravega/gstreamer:$DATE-prod
docker push pravega/gstreamer:$DATE-prod

docker push pravega/gstreamer:latest-prod-dbg
docker tag pravega/gstreamer:latest-prod-dbg pravega/gstreamer:$DATE-prod-dbg
docker push pravega/gstreamer:$DATE-prod-dbg
