mod time_range;
mod message;

use std::collections::HashMap;
use std::net::SocketAddr;
use bytes::Bytes;
use http_body_util::{Empty, Full};
use hyper::{Request, Response};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::{TokioIo};
use tokio::net::TcpListener;
use hyper::{Method, StatusCode};
use http_body_util::{combinators::BoxBody, BodyExt};
use hyper::header::HeaderValue;
use serde_json::{Value};
use crate::message::Message;
use crate::time_range::TimeRange;
use sqlx::{Connection, Executor, MySqlConnection, Row};
use sqlx::migrate::Migrate;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr: SocketAddr = ([127, 0, 0, 1], 8080).into();
    let listener = TcpListener::bind(addr).await?;

    let mut conn = create_db_connection().await;

    sqlx::query("CREATE TABLE IF NOT EXISTS messages (
        id BIGINT PRIMARY KEY AUTO_INCREMENT,
        username VARCHAR(128) NOT NULL,
        message TEXT NOT NULL,
        timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        )").execute(&mut conn).await?;

    loop {
        let (tcp, _) = listener.accept().await?;
        let io = TokioIo::new(tcp);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new().serve_connection(io, service_fn(echo)).await {
                println!("Error serving connection: {:?}", err);
            }
        });
    }

    async fn echo(req: Request<hyper::body::Incoming>) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
        match (req.method(), req.uri().path()) {
            (&Method::GET, "/") => {
                let time_range = match req.uri().query() {
                    Some(query) => parse_query(query),
                    None => Ok(TimeRange {
                        before: None,
                        after: None
                    })
                };

                println!("time_range: {:?}", time_range);

                let response = match time_range {
                    Ok(time_range) => {
                        let messages_from_db = query_db(&time_range);
                        make_get_response(&messages_from_db)
                    },
                    Err(error) => make_error_response(&error, StatusCode::BAD_REQUEST)
                };

                return response;
            },
            (&Method::POST, "/") => {
                let body_string = String::from_utf8(req.collect().await?.to_bytes().to_vec()).unwrap();
                let body_json: Value = serde_json::from_str(&body_string).unwrap();

                let username = body_json.get("username").unwrap().to_string();
                let message = body_json.get("message").unwrap().to_string();

                let mut conn = create_db_connection().await;
                sqlx::query("INSERT INTO messages (username, message) VALUES (?, ?)")
                        .bind(&username)
                        .bind(&message)
                        .execute(&mut conn).await.unwrap();

                Ok(Response::new(empty()))
            },
            _ => {
                make_error_response("", StatusCode::NOT_FOUND)
            }
        }
    }

    async fn create_db_connection() -> MySqlConnection {
        MySqlConnection::connect("mysql://root:qwer123!@localhost:3306/rust_microservice")
            .await
            .unwrap()
    }

    fn empty() -> BoxBody<Bytes, hyper::Error> {
        Empty::<Bytes>::new()
            .map_err(|never| match never {})
            .boxed()
    }
    fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
        Full::new(chunk.into())
            .map_err(|never| match never {})
            .boxed()
    }

    fn parse_query(query: &str) -> Result<TimeRange, String> {
        let args = url::form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect::<HashMap<String, String>>();

        let before = match args.get("before") {
            Some(before) => {
                match before.parse::<i64>() {
                    Ok(before) => Some(before),
                    Err(error) => {
                        return Err(format!("Error parsing 'before': {}", error));
                    }
                }
            },
            None => None
        };

        let after = match args.get("after") {
            Some(after) => {
                match after.parse::<i64>() {
                    Ok(after) => Some(after),
                    Err(error) => {
                        return Err(format!("Error parsing 'after': {}", error));
                    }
                }
            },
            None => None
        };

        Ok(TimeRange {
            before,
            after,
        })
    }

    fn query_db(time_range: &TimeRange) -> Option<Vec<Message>> {
        None
    }

    fn make_get_response(messages: &Option<Vec<Message>>) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
        let response = match messages {
            Some(messages) => {
                let body = render_page(messages);
                let body_len = body.len() as u64;
                let mut res = Response::new(full(body));
                *res.status_mut() = StatusCode::OK;
                res.headers_mut().insert("Content-Length", HeaderValue::from(body_len));
                res
            }
            None => {
                let mut res = Response::new(empty());
                *res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                res
            }
        };

        Ok(response)
    }

    fn make_error_response(error: &str, status_code: StatusCode) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
        let mut res = Response::new(full(String::from(error)));
        *res.status_mut() = status_code;
        Ok(res)
    }

    fn render_page(message: &Vec<Message>) -> String {
        "".to_string()
    }
}