FROM rust

RUN apt-get update && apt-get install -y nodejs npm && npm install -g prisma

WORKDIR /app
COPY . .
RUN cargo build --release
RUN chmod +x docker-entrypoint.sh

ENTRYPOINT ["/app/docker-entrypoint.sh"]
