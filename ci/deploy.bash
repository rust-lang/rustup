#!/bin/bash
# This script is used to do rustup releases. It requires an AWS access token
# allowed to update our S3 buckets and to invalidate CloudFront distributions.
#
# Usage:
#
# 1. Deploy the release on the dev environment:
#    ./deploy.bash dev VERSION_NUMBER
#
# 2. Test everything works correctly:
#    RUSTUP_UPDATE_ROOT=https://dev-static.rust-lang.org/rustup rustup self update
#
# 3. Deploy the release to the prod environment:
#    ./deploy.bash prod VERSION_NUMBER

set -euo pipefail
IFS=$'\n\t'

CI_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd )"
LOCAL_RUSTUP_DIR="local-rustup"

usage() {
    echo "usage: $0 {dev,prod} <version>"
    exit 1
}

run() {
    python3 "${CI_DIR}/sync-dist.py" "$@" --live-run
}

if [[ $# -ne 2 ]]; then
    usage
fi
mode="$1"
version="$2"

case "${mode}" in
    dev)
        # Ask for confirmation before clearing the local directory
        if [[ -e "${LOCAL_RUSTUP_DIR}" ]]; then
            read -rp "The directory ${LOCAL_RUSTUP_DIR} will be removed. Continue (y/n)?" choice
            case "${choice}" in
                y|Y)
                    rm -rf "${LOCAL_RUSTUP_DIR}"
                    ;;
                *)
                    echo "Exiting..."
                    exit 0
            esac
        fi

        run dev-to-local
        run local-to-dev-archives "${version}"
        run update-dev-release "${version}"
        ;;
    prod)
        run local-to-prod-archives "${version}"
        run local-to-prod
        run update-prod-release "${version}"
        ;;
    *)
        usage
        ;;
esac
