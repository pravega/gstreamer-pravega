#!/usr/bin/env bash
set -ex
docker run -it --network host --rm bitnami/kafka:latest kafka-console-consumer.sh \
--bootstrap-server localhost:9092 --topic topic1 --from-beginning
