FROM public.ecr.aws/docker/library/rust:1.57-slim-bullseye as builder
WORKDIR /workspace
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs && cargo build --locked --release && rm src/main.rs target/release/deps/hako_server-*
COPY src ./src/
RUN cargo build --locked --release

FROM public.ecr.aws/debian/debian:bullseye-slim
COPY --from=builder /workspace/target/release/hako-server /usr/local/bin/hako-server
CMD ["hako-server"]
