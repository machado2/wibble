#!/bin/sh
set -eu

cd /app
export LANGUAGE_MODEL="${LANGUAGE_MODEL:-${OPENAI_MODEL:-${OPENROUTER_MODEL:-openai/gpt-4o-mini}}}"
printf '%s\n' 'Applying database migrations...'
prisma migrate deploy --schema=database/prisma/schema.prisma
printf '%s\n' 'Starting Wibble...'
exec /app/target/release/wibble
