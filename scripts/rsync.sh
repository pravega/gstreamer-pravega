#!/usr/bin/env bash
# Copy this repo and related files to a remote system.
set -ex

: ${SSH_HOST?"You must export SSH_HOST"}

rsync -e "ssh ${SSH_OPTS}" -v -r -c --delete --exclude-from .gitignore . ${SSH_HOST}:~/gstreamer-pravega
