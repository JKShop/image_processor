FROM ubuntu as runtime
RUN echo "nameserver 1.1.1.1" > /etc/resolv.conf
RUN apt-get update
RUN apt-get install -y libssl-dev imagemagick
CMD mkdir /app
WORKDIR /app
COPY ./target/release/image_processor /app/image_processor

RUN chmod +x /app/image_processor
ENTRYPOINT ["/app/image_processor"]