FROM alpine:latest AS build
RUN apk add --no-cache rust cargo
WORKDIR /build
RUN mkdir src && echo 'fn main() {panic!()}' > src/main.rs
ADD Cargo.* ./
RUN cargo build -r --no-default-features
ADD templates templates
ADD src src
RUN touch src/main.rs
RUN cargo build -r --no-default-features
FROM alpine:latest
RUN apk add --no-cache libgcc git-daemon
COPY --from=build /build/target/release/clam /clam
USER 1000:1000
WORKDIR /data
EXPOSE 9418
CMD ["git", "daemon", "--export-all", "--reuseaddr", "--base-path=/data"]
