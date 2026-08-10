#!/bin/sh
set -eu

cd /home/fabio/services/wibble/rust

# Convert the legacy Docker Compose database hostname without copying secrets.
case "${DATABASE_URL:-}" in
    *"@wibble-db:"*)
        database_prefix=${DATABASE_URL%%@wibble-db:*}
        database_suffix=${DATABASE_URL#*@wibble-db:}
        export DATABASE_URL="${database_prefix}@127.0.0.1:${database_suffix}"
        ;;
esac

export LANGUAGE_MODEL="${LANGUAGE_MODEL:-${OPENAI_MODEL:-${OPENROUTER_MODEL:-openai/gpt-4o-mini}}}"

/usr/local/bin/prisma migrate deploy --schema=database/prisma/schema.prisma
exec /home/fabio/services/wibble/rust/target/release/wibble
