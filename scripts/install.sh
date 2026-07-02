#!/bin/bash

set -e
SCRIPT_PATH=$(dirname "$0")

$SCRIPT_PATH/install_sdkman_java.sh
$SCRIPT_PATH/install_dynamodb_local.sh
