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
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

mod events;
mod rpc_listener;

struct AppState {
    socket_addresses: Mutex<Vec<SocketAddr>>,
    tx: broadcast::Sender<String>,
}

#[tokio::main]
async fn main() {
    // 10k is the maximum number of clients that can be connected at once.
    let (tx, _rx) = broadcast::channel(10_000);
    let app_state = Arc::new(AppState {
        tx,
        socket_addresses: Mutex::new(Vec::new()),
    });

    let tx = app_state.tx.clone();

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(app_state)
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

    let rpc_listener = RpcListener::try_new(network_id, resolver, tx).unwrap();

    rpc_listener.start().await.unwrap();

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

    // if uncommented, not working thread safety issue, why?
    // let mut socket_addresses_unlocked = state.socket_addresses.lock().unwrap();

    // if socket_addresses_unlocked.contains(&who) {
    //     println!("Client {who} already connected");
    //     return;
    // } else {
    //     println!("Client {who} connected");
    //     socket_addresses_unlocked.push(who);
    // }
    // drop(socket_addresses_unlocked);

    let mut rx = state.tx.subscribe();

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
    let mut socket_addresses_unlocked = state.socket_addresses.lock().unwrap();
    let _ = socket_addresses_unlocked
        .iter()
        .position(|x| *x == who)
        .map(|i| socket_addresses_unlocked.remove(i));
    println!("Websocket context {who} destroyed");
}
