# Use UBI9 minimal image
FROM registry.access.redhat.com/ubi9/ubi-minimal:latest

# Install required dependencies
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

# Copy project files
COPY . .

# Build the application
RUN cargo build --release

# Create a non-root user
RUN useradd -m -u 1000 appuser && \
    chown -R appuser:appuser /app
USER appuser

# Set the entrypoint
ENTRYPOINT ["/app/target/release/grw"]

# Expose port (if needed for future use)
EXPOSE 8080