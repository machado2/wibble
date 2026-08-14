# syntax=docker/dockerfile:1.7
FROM node:22-bookworm-slim@sha256:d649c27dae7ba0137b3cef5dd75baa422c08dc3d9e3fc0c23dfb172dc3cc6436 AS build

ENV PNPM_HOME=/pnpm
ENV PATH=$PNPM_HOME:$PATH
WORKDIR /app

RUN corepack enable && corepack prepare pnpm@11.9.0 --activate
COPY . .
RUN pnpm install --frozen-lockfile --ignore-scripts
RUN DATABASE_URL=postgresql://wibble:build@127.0.0.1:5432/wibble \
  pnpm --filter wibble-web exec prisma generate && \
  DATABASE_URL=postgresql://wibble:build@127.0.0.1:5432/wibble \
  pnpm --filter wibble-worker exec prisma generate
RUN pnpm --filter wibble-web exec openapi \
  --input https://stablehorde.net/api/swagger.json \
  --output ./src/generated/stable-horde-api \
  --name stableHordeClient && \
  pnpm --filter wibble-worker exec openapi \
  --input https://stablehorde.net/api/swagger.json \
  --output ./src/generated/stable-horde-api \
  --name stableHordeClient
RUN --mount=type=secret,id=wibble_config,target=/run/secrets/wibble-config.json,required=true \
  WIBBLE_CONFIG_PATH=/run/secrets/wibble-config.json pnpm typecheck && \
  WIBBLE_CONFIG_PATH=/run/secrets/wibble-config.json pnpm build

FROM node:22-bookworm-slim@sha256:d649c27dae7ba0137b3cef5dd75baa422c08dc3d9e3fc0c23dfb172dc3cc6436

ENV PNPM_HOME=/pnpm
ENV PATH=$PNPM_HOME:$PATH
ENV NODE_ENV=production
ENV WIBBLE_CONFIG_PATH=/run/secrets/wibble-config.json
WORKDIR /app

RUN corepack enable && corepack prepare pnpm@11.9.0 --activate
COPY --from=build --chown=node:node /app /app

USER node
CMD ["pnpm", "--filter", "wibble-web", "start"]
