#!/usr/bin/env bash
set -ex
docker run -it --network host --rm bitnami/kafka:latest kafka-topics.sh --zookeeper localhost:2181 --create --topic topic1 --partitions 1 --replication-factor 1
docker run -it --network host --rm bitnami/kafka:latest kafka-topics.sh --zookeeper localhost:2181 --list
docker run -it --network host --rm bitnami/kafka:latest kafka-console-producer.sh --bootstrap-server localhost:9092 --topic topic1
docker run -it --network host --rm bitnami/kafka:latest kafka-console-consumer.sh --bootstrap-server localhost:9092 --topic topic1 --from-beginning
docker run -it --network host --rm bitnami/kafka:latest bash
