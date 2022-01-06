FROM public.ecr.aws/docker/library/rust:1.57-slim-bullseye as builder
WORKDIR /workspace
RUN rustup component add rustfmt
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs && echo 'fn main() {}' > build.rs && cargo build --locked --release && rm -r src/main.rs target/release/deps/hako_server-* build.rs target/release/build/hako-server-*
COPY build.rs ./
COPY proto ./proto/
COPY src ./src/
RUN cargo build --locked --release

FROM public.ecr.aws/debian/debian:bullseye-slim
COPY --from=builder /workspace/target/release/hako-server /usr/local/bin/hako-server
CMD ["hako-server"]
