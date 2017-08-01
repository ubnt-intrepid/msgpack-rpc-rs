#!/bin/bash

set -euo pipefail

plugin_dir="$(cd $(dirname $BASH_SOURCE); pwd)"

pushd $plugin_dir/../.. > /dev/null

cargo run -p simple -q

popd > /dev/null
