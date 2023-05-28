FROM rust:1.69-slim-bookworm as builder

WORKDIR /usr/src

# Create blank project
RUN USER=root cargo new app

## Install target platform (Cross-Compilation) --> Needed for Alpine
RUN rustup target add x86_64-unknown-linux-musl

# We want dependencies cached, so copy those first.
COPY Cargo.toml Cargo.lock /usr/src/app/

# Set the working directory
WORKDIR /usr/src/app

# This is a dummy build to get the dependencies cached.
RUN cargo build --target x86_64-unknown-linux-musl --release

# Now copy in the rest of the sources
COPY src /usr/src/app/src/

RUN touch /usr/src/app/src/main.rs

RUN cargo build --target x86_64-unknown-linux-musl --release

FROM alpine:3.18
COPY --from=builder /usr/src/app/target/x86_64-unknown-linux-musl/release/tcp-spawner /usr/local/bin

EXPOSE 1337

CMD ["tcp-spawner", "0.0.0.0:1337", "sh"]