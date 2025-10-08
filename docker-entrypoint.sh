#!/bin/sh
set -e

if [ -d database/prisma/migrations ]; then
  echo "Applying migrations with prisma migrate deploy..."
  prisma migrate deploy --schema database/prisma/schema.prisma
else
  echo "No migrations folder detected. Using prisma db push with accept-data-loss to apply schema changes (will apply destructive changes)."
  # `db push` doesn't always drop columns; --accept-data-loss forces destructive changes to match schema.prisma.
  # Use with caution: ensure you have DB backups or use migrations instead.
  prisma db push --schema database/prisma/schema.prisma --accept-data-loss
fi
exec ./target/release/wibble
