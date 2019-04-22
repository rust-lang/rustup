#!/bin/bash
#
# This file is intended to be sourced, so it is not being marked as
# an executable file in git.

# This is from
# https://github.com/travis-ci/travis-build/blob/master/lib/travis/build/bash/travis_setup_env.bash
travis_setup_env() {
  export ANSI_RED='\033[31;1m'
  export ANSI_GREEN='\033[32;1m'
  export ANSI_YELLOW='\033[33;1m'
  export ANSI_RESET='\033[0m'
  export ANSI_CLEAR='\033[0K'
}

# This is from
# https://github.com/travis-ci/travis-build/blob/master/lib/travis/build/bash/travis_fold.bash
travis_fold() {
  local action="${1}"
  local name="${2}"
  printf 'travis_fold:%s:%s\r'"${ANSI_CLEAR}" "${action}" "${name}"
}

# This is modified loop version of
# https://github.com/travis-ci/travis-build/blob/master/lib/travis/build/bash/travis_retry.bash
travis_retry() {
  local result=0
  local count=1
  local max=5
  while [ "${count}" -le "${max}" ]; do
    [ "${result}" -ne 0 ] && {
      printf "${ANSI_RED}"'The command "%s" failed. Retrying, %s of %s.'"${ANSI_RESET}"'\n' "${*}" "${count}" "${max}" >&2
    }
    "${@}" && { result=0 && break; } || result="${?}"
    : $((count=count+1))
    sleep 1
  done

  [ "${count}" -gt "${max}" ] && {
    printf "${ANSI_RED}"'The command "%s" failed %s times.'"${ANSI_RESET}"'\n' "${*}" "${max}" >&2
  }

  return "${result}"
}

# This is from
# https://github.com/travis-ci/travis-build/blob/master/lib/travis/build/bash/
travis_nanoseconds() {
  local cmd='date'
  local format='+%s%N'

  if command -v gdate >/dev/null 2>&1; then
    cmd='gdate'
  elif [ "$(uname)" = Darwin ]; then
    format='+%s000000000'
  fi

  "${cmd}" -u "${format}"
}

# Generate random integer like bash's `$RANDOM`.
travis_generate_rand() {
  shuf -i 0-$((0xffffffff)) -n 1
}

travis_time_start() {
  TRAVIS_TIMER_ID="$(printf '%08x' "$(travis_generate_rand)")"
  TRAVIS_TIMER_START_TIME="$(travis_nanoseconds)"
  export TRAVIS_TIMER_ID TRAVIS_TIMER_START_TIME
  printf 'travis_time:start:%s\r'"${ANSI_CLEAR}" "${TRAVIS_TIMER_ID}"
}

travis_time_finish() {
  travis_timer_end_time="$(travis_nanoseconds)"
  local duration="$((travis_timer_end_time - TRAVIS_TIMER_START_TIME))"
  printf '\ntravis_time:end:%s:start=%s,finish=%s,duration=%s\r'"${ANSI_CLEAR}" \
         "${TRAVIS_TIMER_ID}" "${TRAVIS_TIMER_START_TIME}" \
         "${travis_timer_end_time}" "${duration}"
}

travis_do_cmd() {
  echo "$ ${*}"
  "${@}"
  local result="$?"
  export TRAVIS_TEST_RESULT=$((${TRAVIS_TEST_RESULT:-0} | $((result != 0))))

  if [ "${result}" -eq 0 ]; then
    printf '%b' "${ANSI_GREEN}"
  else
    printf '%b' "${ANSI_RED}"
  fi
  printf 'The command "%s" exited with %d.'"${ANSI_RESET}"'\n' "${*}" "${result}"
  return "$result"
}

travis_setup_env
