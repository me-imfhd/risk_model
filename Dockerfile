# Build stage
FROM rust:1.83-slim-bookworm AS builder

WORKDIR /usr/src/app
COPY . .

# Build the application in release mode
RUN cargo build --release

# Production stage with minimal image
FROM debian:bookworm-slim AS production_base
# Add this line to install OpenSSL
RUN apt-get update && apt-get install -y libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app

FROM production_base AS prod_api
COPY --from=builder /app/target/release/risk_model /usr/local/bin/
CMD ["risk_model"]
