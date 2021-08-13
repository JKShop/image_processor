FROM archlinux as runtime
RUN pacman -S pkg-config openssl imagemagick
CMD mkdir /app
WORKDIR /app
COPY ./target/release/image_processor /app/image_processor

RUN chmod +x /app/image_processor
ENTRYPOINT ["/app/image_processor"]