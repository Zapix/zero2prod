#!/usr/bin/env bash
set -x
set -eo pipefail

RUNNING_CONTAINER=$(docker ps -q --filter "name=redis" --format '{{.ID}}')

if [[ -n $RUNNING_CONTAINER ]]; then
  echo >&2 "There is a running redis container. Stopping it..."
  echo >&2 "docker kill $RUNNING_CONTAINER"
  exit 1
fi

docker run \
  --name "redis_$(date '+%s')" \
  -d \
  -p 6379:6379 \
  redis:alpine