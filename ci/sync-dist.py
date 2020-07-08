# This script is used for syncing parts of the rustup dist server
# between the dev environment (dev-static.rlo), the local machine, and
# the prod environment (static.rlo). It's used during the deployment process.
#
# It is used after a successful CI build on the 'stable' branch.
#
# It does only a few things (this is the release process!):
#
# * Sync dev bins to local host:
#   python sync-dist.py dev-to-local
#
# * Sync local bins to dev archives
#   python sync-dist.py local-to-dev-archives 0.2.0
#
# * Update dev release number
#   python sync-dist.py update-dev-release 0.2.0
#
# Test here with `RUSTUP_UPDATE_ROOT=https://dev-static.rust-lang.org/rustup rustup self update`
#
# * Sync local bins to prod archives
#   python sync-dist.py local-to-prod-archives 0.2.0
#
# * Sync local bins to prod
#   python sync-dist.py local-to-prod
#
# * Update prod release number
#   python sync-dist.py update-prod-release 0.2.0
#
# Run the invalidation in cloudfront-invalidation.txt,
# then tag the release.

from __future__ import print_function

import sys
import os
import subprocess
import shutil

def usage():
    print("usage: sync-dist dev-to-local [--live-run]\n"
           "       sync-dist local-to-dev-archives $version [--live-run]\n"
           "       sync-dist update-dev-release $version [--live-run]\n"
           "       sync-dist local-to-prod-archives $version [--live-run]\n"
           "       sync-dist local-to-prod [--live-run]\n"
           "       sync-dist update-prod-release $version [--live-run]\n")
    sys.exit(1)

command = None
archive_version = None
live_run = False

if len(sys.argv) < 2:
    usage()

command = sys.argv[1]

if not command in ["dev-to-local",
                   "local-to-dev-archives",
                   "update-dev-release",
                   "local-to-prod-archives",
                   "local-to-prod",
                   "update-prod-release"]:
    usage()

if "--live-run" in sys.argv:
    live_run = True
    sys.argv.remove("--live-run")

if "archives" in command or "release" in command:
    if len(sys.argv) != 3:
        usage()
    archive_version = sys.argv[2]
elif len(sys.argv) != 2:
    usage()

dev_s3_bucket = "dev-static-rust-lang-org"
prod_s3_bucket = "static-rust-lang-org"

s3_bucket = dev_s3_bucket
if "prod" in command:
    s3_bucket = prod_s3_bucket

print("s3 bucket: " + s3_bucket)
print("command: " + command)
print("archive version: " + str(archive_version))

# First, deal with the binaries

s3cmd = None
if command == "dev-to-local":
    if os.path.exists("local-rustup/dist"):
        shutil.rmtree("local-rustup/dist")
    os.makedirs("local-rustup/dist")
    s3cmd = "aws s3 cp --recursive s3://{}/rustup/dist/ ./local-rustup/dist/".format(s3_bucket)
elif command == "local-to-dev-archives" \
     or command == "local-to-prod-archives":
    s3cmd = "aws s3 cp --recursive ./local-rustup/dist/ s3://{}/rustup/archive/{}/".format(s3_bucket, archive_version)
elif command == "local-to-prod":
    s3cmd = "aws s3 cp --recursive local-rustup/dist/ s3://{}/rustup/dist/".format(s3_bucket)
elif command == "update-dev-release" \
     or command == "update-prod-release":
    s3cmd = "aws s3 cp ./local-rustup/release-stable.toml s3://{}/rustup/release-stable.toml".format(s3_bucket)
else:
    sys.exit(1)

print("s3 command: {}".format(s3cmd))
print()

# Create the release information
if command == "update-dev-release" \
   or command == "update-prod-release":
    with open("./local-rustup/release-stable.toml", "w") as f:
        f.write("schema-version = '1'\n")
        f.write("version = '{}'\n".format(archive_version))

def run_s3cmd(command):
    s3cmd = command.split(" ")

    if not live_run:
        s3cmd += ["--dryrun"]

    # These are old installer names for compatibility. They don't need to
    # be touched ever again.
    if "cloudfront" not in command:
        s3cmd += ["--exclude=*rustup-setup*"]
    print('executing: ', s3cmd)

    subprocess.check_call(s3cmd)

run_s3cmd(s3cmd)

# Next deal with the rustup-init.sh script and website

if command == "dev-to-local":
    if os.path.exists("local-rustup/rustup-init.sh"):
        os.remove("local-rustup/rustup-init.sh")
    run_s3cmd("aws s3 cp s3://{}/rustup/rustup-init.sh ./local-rustup/rustup-init.sh"
              .format(s3_bucket))
    if os.path.exists("local-rustup/www"):
        shutil.rmtree("local-rustup/www")
    os.makedirs("local-rustup/www")
    run_s3cmd("aws s3 cp --recursive s3://{}/rustup/www/ ./local-rustup/www/"
              .format(s3_bucket))

if command == "local-to-prod":
    run_s3cmd("aws s3 cp ./local-rustup/rustup-init.sh s3://{}/rustup/rustup-init.sh"
              .format(s3_bucket))
    run_s3cmd("aws s3 cp ./local-rustup/rustup-init.sh s3://{}/rustup.sh"
              .format(s3_bucket))
    run_s3cmd("aws s3 cp --recursive ./local-rustup/www/ s3://{}/rustup/www/"
              .format(s3_bucket))
    if live_run:
        # Invalidate sh.rustup.rs
        run_s3cmd("aws cloudfront create-invalidation --distribution-id " +
                  "E70E9RGZ6Q27W --paths /*".format(s3_bucket))
        # Invalidate win.rustup.rs
        run_s3cmd("aws cloudfront create-invalidation --distribution-id " +
                  "E2XBMULPACBLNE --paths /*".format(s3_bucket))
        # Invalidate rustup.rs
        run_s3cmd("aws cloudfront create-invalidation --distribution-id " +
                  "EVJCMYBQ0EX26 --paths /*".format(s3_bucket))

if command == "update-dev-release" and live_run:
    run_s3cmd("aws cloudfront create-invalidation --distribution-id " +
              "E30AO2GXMDY230 --paths /rustup/*".format(s3_bucket))
    # Invalidate dev.rustup.rs
    run_s3cmd("aws cloudfront create-invalidation --distribution-id " +
              "E3OQOQ34607Z0A --paths /*")
if command == "update-prod-release" and live_run:
    run_s3cmd("aws cloudfront create-invalidation --distribution-id " +
              "E3NZU1LCBHH4A4 --paths /rustup/*".format(s3_bucket))
