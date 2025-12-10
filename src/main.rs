use anyhow::Result;
use crossbeam::channel::{unbounded, Receiver};
use fastwebsockets::{handshake, FragmentCollector, Frame, OpCode};
use hyper::header::{CONNECTION, HOST, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE};
use hyper::Request;
use hyper_util::rt::TokioIo;
use rayon::ThreadPoolBuilder;
use rustls_pki_types::ServerName;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

mod binance_ws;
mod orderbook;

use orderbook::Orderbook;

const LAST_TRADE_PRICE: &[u8] = b"last_trade_price";
const ORDERBOOK: &[u8] = b"book";

struct ParsedMessage {
    seq_no: u64,
    orderbook: Orderbook,
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

/// Check if this is the initial orderbook snapshot (starts with '[' and contains "book")
fn is_initial_book(raw: &[u8]) -> bool {
    raw.first() == Some(&b'[') && raw.windows(ORDERBOOK.len()).any(|w| w == ORDERBOOK)
}

/// Strip array wrapper: [{...}] -> {...}
/// We already know it starts with '[' from is_initial_book
fn strip_array_wrapper(raw: &[u8]) -> &[u8] {
    // Skip leading '['
    let start = 1;
    // Find last ']' (might have trailing whitespace/newline)
    let end = raw.iter().rposition(|&b| b == b']').unwrap_or(raw.len());
    &raw[start..end]
}

#[tokio::main]
async fn main() -> Result<()> {
    // binance_ws().await?;

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
        "assets_ids": ["115788731075794337121956742643032236521497269339764102625753674719958328415839"]
    }"#;

    ws.write_frame(Frame::text(fastwebsockets::Payload::Borrowed(
        subscribe_message.as_bytes(),
    )))
    .await?;

    println!("🔌 Connected! Listening for last_trade_price events...\n");
    let (tx, rx) = unbounded::<ParsedMessage>();
    std::thread::spawn(move || orderbook_handling(rx));

    let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
    let mut seq_no: u64 = 0;
    loop {
        let frame = ws.read_frame().await?;

        match frame.opcode {
            OpCode::Close => {
                println!("Connection closed");
                break;
            }
            OpCode::Text => {
                let raw = &frame.payload[..];
                if is_initial_book(raw) {
                    let seq = seq_no;
                    seq_no += 1;

                    // Clone sender (cheap - just Arc clone)
                    let tx = tx.clone();
                    // Strip array wrapper and copy bytes
                    let owned_data = strip_array_wrapper(raw).to_vec();

                    pool.spawn(move || {
                        if let Some(ob) = parse_message(&owned_data) {
                            let _ = tx.send(ParsedMessage {
                                seq_no: seq,
                                orderbook: ob,
                            });
                        }
                    });
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn parse_message(raw: &[u8]) -> Option<Orderbook> {
    match Orderbook::from_bytes(raw) {
        Ok(ob) => Some(ob),
        Err(e) => {
            // Print first 500 chars of raw message to debug
            let preview = String::from_utf8_lossy(&raw[..raw.len().min(500)]);
            eprintln!("Parse error: {}\nRaw: {}", e, preview);
            None
        }
    }
}

fn orderbook_handling(rx: Receiver<ParsedMessage>) {
    let mut buffer = BTreeMap::new();
    let mut next_seq: u64 = 0;

    while let Ok(msg) = rx.recv() {
        buffer.insert(msg.seq_no, msg.orderbook);

        while let Some(ob) = buffer.remove(&next_seq) {
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("📊 Orderbook Update #{}", next_seq);
            println!("   Market: {}", ob.market);
            println!("   Asset:  {}", &ob.asset_id[..20]);
            println!("   Time:   {}", ob.timestamp);
            if let Some((bid, bid_sz)) = ob.best_bid() {
                println!("   Best Bid: {} (size: {})", bid, bid_sz);
            }
            if let Some((ask, ask_sz)) = ob.best_ask() {
                println!("   Best Ask: {} (size: {})", ask, ask_sz);
            }
            if let Some(spread) = ob.spread() {
                println!("   Spread:   {}", spread);
            }
            next_seq += 1;
        }
    }
}
