# syntax=docker/dockerfile:1.6

# Build stage
FROM registry.access.redhat.com/ubi9/ubi-minimal:latest AS builder

# Install required dependencies for building
RUN microdnf update -y && \
    microdnf install -y \
        gcc \
        git \
        pkg-config \
        openssl-devel && \
    microdnf clean all

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Create app directory
WORKDIR /app

# 1) Pre-cache dependencies using an empty main so Docker layer cache can be reused
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src \
 && echo 'fn main() { println!("placeholder build to cache dependencies"); }' > src/main.rs

# Use BuildKit cache mounts for registry, git, and target to persist between CI runs
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release

# 2) Now copy the real source and do the actual build; this should reuse cached deps
RUN rm -rf src
COPY src ./src

RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
    cp /app/target/release/grw /grw

# Runtime stage - minimal UBI9 image
FROM registry.access.redhat.com/ubi9/ubi-minimal:latest

# Install only runtime dependencies
RUN microdnf update -y && \
    microdnf install -y \
        libgcc \
        openssl-libs && \
    microdnf clean all

# Create app directory and user
WORKDIR /app
RUN useradd -m -u 1000 appuser

# Copy the binary from the builder stage
COPY --from=builder /grw /app/grw

# Set permissions
RUN chown -R appuser:appuser /app
USER appuser

# Set the entrypoint
ENTRYPOINT ["/app/grw"]

# Expose port (if needed for future use)
EXPOSE 8080
