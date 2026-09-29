#
# DEEPWELL build
#

FROM rust:latest AS rust

# Install system dependencies
RUN apt update
RUN apt install -y build-essential libmagic-dev pkg-config

# Copy source
RUN mkdir /src
COPY ./deepwell /src/deepwell
COPY ./locales /src/locales
WORKDIR /src/deepwell

# Cache rust dependencies
RUN mkdir .cargo && cargo vendor .cargo/vendor > .cargo/config.toml

# Build deepwell server
RUN cargo build --release

#
# Final image
#

FROM debian:latest

ENV LOCALIZATION_PATH="/opt/locales"

RUN apt update
RUN apt install -y curl libmagic1 libmagic-mgc
COPY --from=rust /src/deepwell/target/release/deepwell /usr/local/bin/deepwell
COPY ./install/prod/deepwell/health-check.sh /bin/wikijump-health-check
COPY ./install/prod/deepwell/config.toml /etc/deepwell.toml
COPY ./locales/fluent /opt/locales/fluent

USER daemon
ENV PORT=2747
EXPOSE 2747
CMD ["/usr/local/bin/deepwell", "/etc/deepwell.toml"]
