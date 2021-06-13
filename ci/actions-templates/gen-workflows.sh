#!/bin/sh

INPATH=$(dirname "$0")
OUTPATH="${INPATH}/../../.github/workflows"

gen_workflow () {
    grep -v "skip-$2" "$INPATH/$1-template.yaml" > "$OUTPATH/$1-on-$2.yaml"
}

mkdir -p "$OUTPATH"

# macOS only has a single target so single flow
gen_workflow macos-builds all

gen_workflow windows-builds pr
gen_workflow windows-builds master
gen_workflow windows-builds stable

gen_workflow linux-builds pr
gen_workflow linux-builds master
gen_workflow linux-builds stable

# freebsd
gen_workflow freebsd-builds pr
gen_workflow freebsd-builds master
gen_workflow freebsd-builds stable

# The clippy checks only have a single target and thus single flow
gen_workflow centos-fmt-clippy all

