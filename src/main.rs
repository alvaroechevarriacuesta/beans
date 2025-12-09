use anyhow::Result;
use fastwebsockets::{handshake, FragmentCollector, Frame, OpCode};
use hyper::header::{CONNECTION, HOST, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE};
use hyper::Request;
use hyper_util::rt::TokioIo;
use rustls_pki_types::ServerName;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use crossbeam::channel::{unbounded, Receiver, Sender};

const LAST_TRADE_PRICE: &[u8] = b"last_trade_price";

struct ParsedMessage {
    seq_no: u64,
    data: String,
}

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

/// Check if raw bytes contain the pattern we care about
fn is_last_trade_price(raw: &[u8]) -> bool {
    raw.windows(LAST_TRADE_PRICE.len())
        .any(|w| w == LAST_TRADE_PRICE)
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
        "assets_ids": ["55629675658531796836270928055107359815299274196063137467797078435252406021824"]
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
                let raw = &frame.payload[..];
                if is_last_trade_price(raw) {
                    let text = String::from_utf8_lossy(raw);
                    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                    println!("💰 LAST TRADE PRICE:");
                    println!("{}", text);
                    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                }
            }
            _ => {}
        }
    }

    Ok(())
}
