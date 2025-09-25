#!/usr/bin/env bash
set -euo pipefail

CONN_STRING=${1:-"postgres://postgres:postgres@127.0.0.1:5432/postgres"}

psql "$CONN_STRING" -f "$(dirname "$0")/reset_dev_db.sql"
