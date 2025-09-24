#!/bin/sh
set -e

if [ -d database/prisma/migrations ]; then
  prisma migrate deploy --schema database/prisma/schema.prisma
else
  prisma db push --schema database/prisma/schema.prisma
fi
exec ./target/release/wibble
