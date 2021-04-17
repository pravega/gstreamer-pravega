#!/usr/bin/env bash
#
# Use this with rotatelogs to compress rotated log files.
# Example: mycmd |& rotatelogs -L ${LOG_FILE} -p rotatelogs-compress.sh ${LOG_FILE} 1G

if [[ ! -z "$2" ]]; then
    gzip "$2"
fi
