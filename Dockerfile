FROM debian as runtime
RUN apt-get update
RUN apt-get install -y libssl-dev
CMD mkdir /app
WORKDIR /app
COPY ./target/release/coordinator /app/coordinator

RUN chmod +x /app/coordinator
ENTRYPOINT ["/app/coordinator"]