#![allow(unused)]

use async_http_proxy::http_connect_tokio;
use futures::{SinkExt, StreamExt};
use hudsucker::{
    certificate_authority::RcgenAuthority,
    rustls,
    tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message},
};
use rustls_pemfile as pemfile;
use std::sync::atomic::Ordering;
use tokio::net::TcpStream;

mod common;

fn build_ca() -> RcgenAuthority {
    let mut private_key_bytes: &[u8] = include_bytes!("../examples/ca/hudsucker.key");
    let mut ca_cert_bytes: &[u8] = include_bytes!("../examples/ca/hudsucker.cer");
    let private_key = rustls::PrivateKey(
        pemfile::pkcs8_private_keys(&mut private_key_bytes)
            .expect("Failed to parse private key")
            .remove(0),
    );
    let ca_cert = rustls::Certificate(
        pemfile::certs(&mut ca_cert_bytes)
            .expect("Failed to parse CA certificate")
            .remove(0),
    );

    RcgenAuthority::new(private_key, ca_cert, 1_000)
        .expect("Failed to create Certificate Authority")
}

#[tokio::test]
async fn http() {
    let (proxy_addr, _, websocket_handler, stop_proxy) = common::start_proxy(build_ca()).unwrap();
    let (server_addr, stop_server) = common::start_http_server().unwrap();

    let mut stream = TcpStream::connect(proxy_addr).await.unwrap();
    http_connect_tokio(
        &mut stream,
        &server_addr.ip().to_string(),
        server_addr.port(),
    )
    .await
    .unwrap();

    let (mut ws, _) = tokio_tungstenite::client_async(format!("ws://{}", server_addr), stream)
        .await
        .unwrap();

    ws.send(Message::Text("hello".to_owned())).await.unwrap();

    let msg = ws.next().await.unwrap().unwrap();

    assert_eq!(msg.to_string(), common::WORLD);
    assert_eq!(websocket_handler.message_counter.load(Ordering::Relaxed), 2);

    stop_server.send(()).unwrap();
    stop_proxy.send(()).unwrap();
}

#[tokio::test]
async fn https() {
    let (proxy_addr, _, websocket_handler, stop_proxy) = common::start_proxy(build_ca()).unwrap();
    let (server_addr, stop_server) = common::start_https_server(build_ca()).await.unwrap();

    let mut stream = TcpStream::connect(proxy_addr).await.unwrap();
    http_connect_tokio(&mut stream, "localhost", server_addr.port())
        .await
        .unwrap();

    let (mut ws, _) = tokio_tungstenite::client_async_tls_with_config(
        format!("wss://localhost:{}", server_addr.port()),
        stream,
        None,
        Some(common::tokio_tungstenite_connector()),
    )
    .await
    .unwrap();

    ws.send(Message::Text("hello".to_owned())).await.unwrap();

    let msg = ws.next().await.unwrap().unwrap();

    assert_eq!(msg.to_string(), common::WORLD);
    assert_eq!(websocket_handler.message_counter.load(Ordering::Relaxed), 2);

    stop_server.send(()).unwrap();
    stop_proxy.send(()).unwrap();
}

#[tokio::test]
async fn noop() {
    let (proxy_addr, stop_proxy) = common::start_noop_proxy(build_ca()).unwrap();
    let (server_addr, stop_server) = common::start_http_server().unwrap();

    let mut stream = TcpStream::connect(proxy_addr).await.unwrap();
    http_connect_tokio(
        &mut stream,
        &server_addr.ip().to_string(),
        server_addr.port(),
    )
    .await
    .unwrap();

    let (mut ws, _) = tokio_tungstenite::client_async(format!("ws://{}", server_addr), stream)
        .await
        .unwrap();

    ws.send(Message::Text("hello".to_owned())).await.unwrap();
    let msg = ws.next().await.unwrap().unwrap();

    assert_eq!(msg.to_string(), common::WORLD);

    stop_server.send(()).unwrap();
    stop_proxy.send(()).unwrap();
}
