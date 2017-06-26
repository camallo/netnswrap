FROM ekidd/rust-musl-builder:1.18.0
COPY . /home/rust/src/
RUN cargo build --locked --release --target x86_64-unknown-linux-musl 

FROM scratch
COPY --from=0 /home/rust/src/target/x86_64-unknown-linux-musl/release/netnswrap /
USER 0
CMD ["/netnswrap", "--help"]  
