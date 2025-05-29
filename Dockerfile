FROM alpine:latest AS build
RUN apk add --no-cache rust cargo
ADD Cargo.* /build/
ADD templates /build/templates
ADD src /build/src
WORKDIR /build
RUN cargo build -r --no-default-features
FROM alpine:latest
RUN apk add --no-cache libgcc git-daemon
COPY --from=build /build/target/release/clam /clam
USER 1000:1000
WORKDIR /data
EXPOSE 9418
CMD ["git", "daemon", "--export-all", "--reuseaddr", "--base-path=/data"]
