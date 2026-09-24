#
# WWS build
#

FROM rust:latest AS rust

# Install files
COPY ./install/prod/wws/health-check.sh /usr/local/bin/wikijump-health-check

# Copy source
RUN mkdir /src
COPY ./wws /src/wws
COPY ./deepwell/vendor/wikidot-normalize /src/deepwell/vendor/wikidot-normalize
WORKDIR /src/wws

# Cache rust dependencies
RUN mkdir .cargo && cargo vendor > .cargo/config.toml

# Build wws
RUN cargo build --release

#
# Final image
#

FROM debian:latest

# Install dependencies.
RUN apt update && apt install -y curl

# Install files
COPY --from=rust /src/wws/target/release/wws /usr/local/bin/wws
COPY ./install/prod/wws/health-check.sh /usr/local/bin/wikijump-health-check

USER daemon
EXPOSE 3466
CMD ["/usr/local/bin/wws"]
