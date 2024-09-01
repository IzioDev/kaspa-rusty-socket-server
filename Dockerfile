FROM rust:1 as build-env
WORKDIR /app
COPY . /app
RUN cargo build --release

FROM gcr.io/distroless/cc-debian12
COPY --from=build-env /app/target/release/kaspa-rusty-socket-server /
CMD ["./kaspa-rusty-socket-server"]
