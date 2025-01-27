FROM alpine as builder

RUN apk upgrade && apk add \
    # rustup
    curl \
    gcc \
    # required for compiling a lot of crates
    musl-dev \
    # init system
    dumb-init \
    && curl https://sh.rustup.rs -sSf | sh -s -- --profile minimal --default-toolchain nightly -y

ARG FEATURES="error_reporting"

WORKDIR /build

COPY ./Cargo.lock .
COPY ./Cargo.toml .

# twilight-rs/http-proxy build trick:
# https://github.com/twilight-rs/http-proxy/blob/f7ea681fa4c47b59692827974cd3a7ceb2125161/Dockerfile#L40-L75
RUN mkdir src \
    && echo 'fn main() {}' > src/main.rs \
    && source $HOME/.cargo/env \
    && cargo build --release --features="$FEATURES" \
    && rm -f target/release/deps/pink_server*

COPY src src

RUN source $HOME/.cargo/env \
    && cargo build --release --features="$FEATURES" \
    && cp target/release/pink-server pink-server

FROM scratch

COPY --from=builder /usr/bin/dumb-init /bin/dumb-init
COPY --from=builder /build/pink-server /usr/bin/pink-server

WORKDIR /app

# TODO: user

EXPOSE 8001

ENTRYPOINT ["/bin/dumb-init", "--", "pink-server"]