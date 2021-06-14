# image_processor
- Converts images to webp as a service
## Usage

Set env variables:
```dotenv
IMG_PROCESSOR.ADDR=0.0.0.0
IMG_PROCESSOR.PORT=5647
SNOWFLAKE.COORDINATOR=https://coordinator.example.com
```

```shell
cargo run --release
```
