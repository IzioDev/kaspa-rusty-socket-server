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
use dotenvy::dotenv;
use events::events::{ExplorerEvent, ExplorerLastestBlocks, ExplorerMessage};
use futures::{sink::SinkExt, stream::StreamExt};
use kaspa_wrpc_client::Resolver;
use rpc_listener::RpcListener;
use settings::AppSettings;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;
use tokio::sync::{broadcast, Mutex};
use tower_http::cors::CorsLayer;

mod events;
mod rpc_listener;
mod settings;

struct AppState {
    tx_broadcast_sockets: broadcast::Sender<Arc<String>>,
    // store in the reversed order (first is the oldest block)
    latest_blocks: RwLock<Vec<Arc<String>>>,
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let app_settings = AppSettings::new();

    // 10k is the maximum number of clients that can be connected at once.
    let (tx_broadcast_sockets, _rx_broadcast_scokets) = broadcast::channel(app_settings.max_clients);

    let (tx_block_added, _rx_block_added) = broadcast::channel(1);

    let app_state = Arc::new(AppState {
        tx_broadcast_sockets: tx_broadcast_sockets.clone(),
        latest_blocks: RwLock::new(Vec::new()),
    });

    let app = Router::new()
        .route(app_settings.ws_path.as_str(), get(ws_handler))
        .with_state(app_state.clone())
        .layer(
            CorsLayer::new()
                .allow_origin("*".parse::<HeaderValue>().unwrap())
                .allow_methods([Method::GET]),
        );

    let listener = tokio::net::TcpListener::bind(
        format!("{}:{}", app_settings.ws_host, app_settings.ws_port)
            .parse::<SocketAddr>()
            .unwrap(),
    )
    .await
    .unwrap();

    let resolver = Resolver::default();

    let rpc_listener = RpcListener::try_new(
        app_settings.network_id,
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
            let mut latest_blocks_unlocked = app_state.latest_blocks.write().await;
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
    let (tx, mut receiver) = socket.split();

    let arc_tx = Arc::new(Mutex::new(tx));
    let arc_tx_2 = arc_tx.clone();

    let mut rx = state.tx_broadcast_sockets.subscribe();

    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let mut sender = arc_tx.lock().await;
            // In any websocket error, break loop.
            if sender.send(Message::Text(msg.to_string())).await.is_err() {
                break;
            }

            drop(sender);
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            if text.eq("get_last_blocks") {
                let mut sender = arc_tx_2.lock().await;
                // on initial connection, send latest blocks
                let latest_blocks = state.latest_blocks.read().await;
                let latest_blocks_as_string_vec = latest_blocks
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<String>>();

                let mut vec: Vec<ExplorerMessage> = Vec::new();
                vec.push(ExplorerMessage::EventType("last-blocks".to_owned()));
                vec.push(ExplorerMessage::ExplorerEvent(ExplorerEvent::LatestBlocks(
                    ExplorerLastestBlocks::Data(latest_blocks_as_string_vec),
                )));

                let json = serde_json::to_string(&vec).unwrap();

                sender.send(Message::Text(json)).await.unwrap();

                drop(latest_blocks);
            } else {
                println!("{who} sent: {text}");
            }
        }
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    };

    // connection closed
    println!("Websocket context {who} destroyed");
}
