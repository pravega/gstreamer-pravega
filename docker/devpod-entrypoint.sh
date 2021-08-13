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

set -ex
export >> ${HOME}/.profile
mkdir -p ${HOME}/.ssh

ssh-keygen -f ${HOME}/.ssh/ssh_host_rsa_key -N '' -t rsa
ssh-keygen -f ${HOME}/.ssh/ssh_host_ecdsa_key -N '' -t ecdsa
ssh-keygen -f ${HOME}/.ssh/ssh_host_ed25519_key -N '' -t ed25519

# This will copy authorized_keys.
if [[ -e /tmp/ssh-configmap ]]; then
    cp /tmp/ssh-configmap/* ${HOME}/.ssh/
fi

chmod 700 ${HOME}/.ssh
chmod 600 ${HOME}/.ssh/*

/usr/sbin/sshd -D -f ${HOME}/.ssh/sshd_config
