#![forbid(unsafe_code)]
#![deny(clippy::needless_borrow, clippy::panic, clippy::unwrap_used)]
#![deny(unused_imports)]
#![forbid(missing_docs)]
//! This crate provides an server, who converts incoming images to webp and stores them into an s3 bucket

use actix_web::http::StatusCode;
use actix_web::{web, App, Error, HttpResponse, HttpServer};

use derive_more::{Display, From};
use dotenv::dotenv;

use futures_lite::StreamExt;
use image::{EncodableLayout, ImageError};
use once_cell::sync::{Lazy, OnceCell};

use serde::Serialize;
use snowflake::Snowflake;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Mutex;
use std::time::Duration;
use std::{env, thread};

use image::io::Reader as ImageReader;

/// Error wrapper for all errors, that could be thrown by the server
#[derive(Display, From, Debug)]
enum ImageProcessorError {
    /// std::io::Error
    IO(std::io::Error),
    /// ImageError
    Img(ImageError),
    /// actix_web::Error
    Actix(actix_web::Error),
}
impl std::error::Error for ImageProcessorError {}

#[derive(Debug, Clone, PartialEq, Serialize)]
enum STATUS {
    Waiting,
    Processing,
    Finished,
    Failed(String),
}

#[derive(Debug, Clone, Serialize)]
struct WorkObject {
    item_id: String,
    image_data: Vec<u8>,
    status: STATUS,
}

static WORK_QUEUE: Lazy<Mutex<HashMap<String, WorkObject>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

static THREAD_STARTER: OnceCell<bool> = OnceCell::new();
const THREAD_MAX: usize = 64;

fn thread_worker() {
    loop {
        thread::sleep(Duration::from_secs(1));
        let _new_wq: (String, WorkObject) = {
            let mut wq = match WORK_QUEUE.lock() {
                Ok(v) => v,
                Err(e) => {
                    #[allow(clippy::panic)]
                        {
                            panic!("{:?}", e)
                        }
                }
            };
            if wq.is_empty() {
                continue;
            }

            let awaiting_wq = wq
                .iter()
                .filter(|w| w.1.status == STATUS::Waiting)
                .collect::<Vec<(&String, &WorkObject)>>();

            let work = match awaiting_wq.first() {
                None => continue,
                Some(v) => (
                    v.0.clone(),
                    WorkObject {
                        item_id: v.1.item_id.clone(),
                        image_data: v.1.image_data.clone(),
                        status: v.1.status.clone(),
                    },
                ),
            };
            wq.insert(
                work.0.clone(),
                WorkObject {
                    item_id: work.1.item_id.clone(),
                    image_data: vec![],
                    status: STATUS::Processing,
                },
            );

            work
        };
        log::debug!("Thread got work {:?}", thread::current().id());

        let loaded_img = match ImageReader::new(Cursor::new(_new_wq.1.image_data.as_slice()))
            .with_guessed_format()
        {
            Ok(v) => match v.decode() {
                Ok(vx) => vx,
                Err(e) => {
                    log::error!("{:?}", e);
                    {
                        let mut wq = match WORK_QUEUE.lock() {
                            Ok(v) => v,
                            Err(e) => {
                                #[allow(clippy::panic)]
                                    {
                                        panic!("{:?}", e)
                                    }
                            }
                        };
                        wq.insert(
                            _new_wq.0,
                            WorkObject {
                                item_id: _new_wq.1.item_id,
                                image_data: _new_wq.1.image_data,
                                status: STATUS::Failed(format!("{:?}", e)),
                            },
                        );
                    }
                    continue;
                }
            },
            Err(e) => {
                log::error!("{:?}", e);
                {let mut wq = match WORK_QUEUE.lock() {
                    Ok(v) => v,
                    Err(e) => {
                        #[allow(clippy::panic)]
                            {
                                panic!("{:?}", e)
                            }
                    }
                };
                    wq.insert(
                        _new_wq.0,
                        WorkObject {
                            item_id: _new_wq.1.item_id,
                            image_data: _new_wq.1.image_data,
                            status: STATUS::Failed(format!("{:?}", e)),
                        },
                    );
                }
                continue;
            }
        };

        let webp_img = webp::Encoder::from_image(&loaded_img).encode(75f32);
        {let mut wq = match WORK_QUEUE.lock() {
            Ok(v) => v,
            Err(e) => {
                #[allow(clippy::panic)]
                    {
                        panic!("{:?}", e)
                    }
            }
        };
            wq.insert(
                _new_wq.0.clone(),
                WorkObject {
                    item_id: _new_wq.1.item_id,
                    image_data: webp_img.as_bytes().to_vec(),
                    status: STATUS::Finished,
                },
            );
        }
        log::debug!("Processed image {}", _new_wq.0);
    }
}

fn start_threads() {
    for _ in 0..THREAD_MAX {
        thread::spawn(thread_worker);
    }
}

async fn add_to_queue(
    web::Path((region,)): web::Path<(u8,)>,
    web::Path((item_id,)): web::Path<(String,)>,
    mut body: web::Payload,
) -> Result<HttpResponse, Error> {
    THREAD_STARTER.get_or_init(|| {
        start_threads();
        true
    });

    let mut bytes = web::BytesMut::new();
    while let Some(item) = body.next().await {
        bytes.extend_from_slice(&item?);
    }

    let snow = {
        let mut work_queue = match WORK_QUEUE.lock() {
            Ok(v) => v,
            Err(e) => {
                #[allow(clippy::panic)]
                    {
                        panic!("{:?}", e)
                    }
            }
        };

        let snow = Snowflake::new(region).await.to_string();
        work_queue.insert(
            snow.clone(),
            WorkObject {
                item_id,
                image_data: bytes.as_bytes().to_vec(),
                status: STATUS::Waiting,
            },
        );
        snow
    };

    HttpResponse::build(StatusCode::OK).body(snow).await
}

async fn get_image_status(
    web::Path((snowflake_id,)): web::Path<(String,)>,
) -> Result<HttpResponse, Error> {
    {
        let mut wq = match WORK_QUEUE.lock() {
            Ok(v) => v,
            Err(e) => {
                #[allow(clippy::panic)]
                    {
                        panic!("{:?}", e)
                    }
            }
        };

        match wq.get(&snowflake_id) {
            None => HttpResponse::build(StatusCode::NOT_FOUND).await,
            Some(v) => match &v.status {
                STATUS::Waiting | STATUS::Processing => {
                    let mut vc = v.clone();
                    vc.image_data = vec![];
                    let encoded = serde_json::to_string(&vc)?;
                    HttpResponse::build(StatusCode::OK).body(encoded).await
                }
                STATUS::Finished => {
                    let encoded = serde_json::to_string(&v)?;
                    wq.remove(&snowflake_id);

                    HttpResponse::build(StatusCode::OK).body(encoded).await
                }
                STATUS::Failed(e) => {
                    HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(format!("{:?}", e))
                        .await
                }
            },
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();
    match dotenv() {
        Ok(_) => {}
        Err(_e) => {
            log::warn!("No .env var found !")
        }
    }

    let server = HttpServer::new(move || {
        let app = App::new();
        app.service(web::resource("/{region}/{id}").route(web::post().to(add_to_queue)))
            .service(web::resource("/status//{snowflake_id}").route(web::get().to(get_image_status)))
    })
    .bind(format!(
        "{}:{}",
        env::var("IMG_PROCESSOR.ADDR").expect("Env key IMG_PROCESSOR.ADDR not set"),
        env::var("IMG_PROCESSOR.PORT").expect("Env key IMG_PROCESSOR.PORT not set")
    ))?
    .run();
    log::info!("Spawned server");
    server.await
}
