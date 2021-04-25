#!/bin/bash

#
# Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#

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
