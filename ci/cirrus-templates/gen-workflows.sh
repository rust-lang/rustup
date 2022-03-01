#!/bin/sh

INPATH=$(dirname "$0")
OUTPATH="${INPATH}/../.."

cp freebsd.yaml "$OUTPATH/.cirrus.yml"
