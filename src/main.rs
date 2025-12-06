use anyhow::Result;
use fastwebsockets::{handshake, FragmentCollector, Frame, OpCode};
use hyper::header::{CONNECTION, HOST, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE};
use hyper::Request;
use hyper_util::rt::TokioIo;
use rustls_pki_types::ServerName;
use serde_json::Value;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

struct SpawnExecutor;

impl<Fut> hyper::rt::Executor<Fut> for SpawnExecutor
where
    Fut: std::future::Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    fn execute(&self, fut: Fut) {
        tokio::task::spawn(fut);
    }
}

/// Process and display last_trade_price events
fn process_event(json_str: &str) {
    let Ok(value) = serde_json::from_str::<Value>(json_str) else {
        return;
    };

    // Check if this is a last_trade_price event
    if value.get("event_type").and_then(|v| v.as_str()) == Some("last_trade_price") {
        let price = value.get("price").and_then(|v| v.as_str()).unwrap_or("?");
        let size = value.get("size").and_then(|v| v.as_str()).unwrap_or("?");
        let asset_id = value
            .get("asset_id")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let timestamp = value
            .get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("?");

        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("💰 LAST TRADE PRICE");
        println!("   Price:     {}", price);
        println!("   Size:      {}", size);
        println!("   Asset ID:  {}...", &asset_id[..20.min(asset_id.len())]);
        println!("   Timestamp: {}", timestamp);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let host = "ws-subscriptions-clob.polymarket.com";
    let path = "/ws/market";
    let port = 443;

    // Connect TCP
    let tcp_stream = TcpStream::connect((host, port)).await?;

    // Setup TLS with rustls
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let connector = TlsConnector::from(Arc::new(config));
    let domain = ServerName::try_from(host.to_string())?;
    let tls_stream = connector.connect(domain, tcp_stream).await?;

    // Double-wrap: TokioIo makes hyper traits work, but we need the inner to have tokio traits
    // Actually, pass TokioIo<TokioIo<...>> - the inner TokioIo gives tokio traits, outer gives hyper traits
    let io = TokioIo::new(TokioIo::new(tls_stream));

    // Build WebSocket upgrade request
    let req = Request::builder()
        .method("GET")
        .uri(path)
        .header(HOST, host)
        .header(UPGRADE, "websocket")
        .header(CONNECTION, "Upgrade")
        .header(SEC_WEBSOCKET_KEY, handshake::generate_key())
        .header(SEC_WEBSOCKET_VERSION, "13")
        .body(http_body_util::Empty::<bytes::Bytes>::new())?;

    // Perform the WebSocket handshake
    let (ws, _) = handshake::client(&SpawnExecutor, req, io).await?;
    let mut ws = FragmentCollector::new(ws);

    // Subscribe to market updates
    let subscribe_message = r#"{
        "type": "market",
        "assets_ids": ["71358653749567369751762906175193924319733398285624296258738807024936977343791"]
    }"#;

    ws.write_frame(Frame::text(fastwebsockets::Payload::Borrowed(
        subscribe_message.as_bytes(),
    )))
    .await?;

    println!("🔌 Connected! Listening for last_trade_price events...\n");

    loop {
        let frame = ws.read_frame().await?;

        match frame.opcode {
            OpCode::Close => {
                println!("Connection closed");
                break;
            }
            OpCode::Text => {
                let txt = String::from_utf8_lossy(&frame.payload);
                process_event(&txt);
            }
            _ => {}
        }
    }

    Ok(())
}
