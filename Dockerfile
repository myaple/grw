# Build stage
FROM registry.access.redhat.com/ubi9/ubi-minimal:latest as builder

# Install required dependencies for building
RUN microdnf update -y && \
    microdnf install -y \
        gcc \
        git \
        pkg-config \
        openssl-devel && \
    microdnf clean all

# Create app directory
WORKDIR /app

# Copy project files
COPY . .

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \
    . $HOME/.cargo/env

# Build the application with target directory cache
RUN --mount=type=cache,target=/app/target,id=target-cache \
    . $HOME/.cargo/env && \
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
