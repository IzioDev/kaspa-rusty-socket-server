use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        ConnectInfo, State,
    },
    http::{HeaderValue, Method},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use kaspa_wrpc_client::{
    prelude::{NetworkId, NetworkType},
    Resolver,
};
use rpc_listener::RpcListener;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

mod events;
mod rpc_listener;

struct AppState {
    tx_broadcast_sockets: broadcast::Sender<String>,

    // store in the reversed order (first is the oldest block)
    latest_blocks: Mutex<Vec<String>>,
}

#[tokio::main]
async fn main() {
    // 10k is the maximum number of clients that can be connected at once.
    let (tx_broadcast_sockets, _rx_broadcast_scokets) = broadcast::channel(10_000);

    let (tx_block_added, _rx_block_added) = broadcast::channel(1);

    let app_state = Arc::new(AppState {
        tx_broadcast_sockets: tx_broadcast_sockets.clone(),
        latest_blocks: Mutex::new(Vec::new()),
    });

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(app_state.clone())
        .layer(
            CorsLayer::new()
                .allow_origin("*".parse::<HeaderValue>().unwrap())
                .allow_methods([Method::GET]),
        );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3002")
        .await
        .unwrap();

    let resolver = Resolver::default();

    let network_id = NetworkId::new(NetworkType::Mainnet);

    // for TN testings
    // let network_id = NetworkId::with_suffix(NetworkType::Testnet, 11);

    let rpc_listener = RpcListener::try_new(
        network_id,
        resolver,
        tx_broadcast_sockets.clone(),
        tx_block_added.clone(),
    )
    .unwrap();

    rpc_listener.start().await.unwrap();

    // listen to new block event, and populate in app state
    tokio::spawn(async move {
        let mut rx = tx_block_added.subscribe();
        while let Ok(block_as_json) = rx.recv().await {
            let mut latest_blocks_unlocked = app_state.latest_blocks.lock().await;
            latest_blocks_unlocked.push(block_as_json);

            let length = latest_blocks_unlocked.len();
            if length > 20 {
                // remove the first element
                latest_blocks_unlocked.remove(0);
            }

            // release lock on mutex
            drop(latest_blocks_unlocked);
        }
    });

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    println!("{addr} connected.");
    ws.on_upgrade(move |socket| handle_socket(socket, addr, state))
}

async fn handle_socket(socket: WebSocket, who: SocketAddr, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    // on initial connection, send latest blocks
    let latest_blocks = state.latest_blocks.lock().await;
    for block in latest_blocks.iter() {
        sender.send(Message::Text(block.clone())).await;
    }
    drop(latest_blocks);

    let mut rx = state.tx_broadcast_sockets.subscribe();

    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            // In any websocket error, break loop.
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            print!("{who} sent: {text}");
        }
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    };

    // connection closed
    println!("Websocket context {who} destroyed");
}
