#!/usr/bin/env bash

#
# Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#

set -ex
docker run -it --network host --rm bitnami/kafka:latest kafka-console-consumer.sh \
--bootstrap-server localhost:9092 --topic topic1 --from-beginning
