//! WebSocket bridge between Houston App and private cloud engines.

use crate::agents;
use crate::audit;
use crate::auth::Principal;
use crate::db::Db;
use crate::entitlements;
use crate::error::{ApiError, ApiResult};
use crate::runtime_wake;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct RuntimeTarget {
    internal_url: String,
    engine_token: String,
}

pub async fn proxy_ws(
    State(state): State<Arc<crate::state::AppState>>,
    principal: Principal,
    Path(agent_id): Path<Uuid>,
    ws: WebSocketUpgrade,
) -> ApiResult<Response> {
    agents::assert_agent_access(&state.db, &principal, agent_id, "operator").await?;
    entitlements::assert_active(&state.db, &principal).await?;
    runtime_wake::ensure_agent_awake(&state.db, state.runtime.as_ref(), agent_id).await?;
    let runtime = load_runtime(&state.db, agent_id).await?;
    let engine_url = format!(
        "{}/v1/ws",
        runtime
            .internal_url
            .replace("http://", "ws://")
            .replace("https://", "wss://")
    );
    let token = runtime.engine_token;
    let db = state.db.clone();
    let org_id = principal.org_id;
    let user_id = principal.user_id;
    Ok(ws.on_upgrade(move |client| async move {
        if let Err(e) = bridge(client, &engine_url, &token).await {
            tracing::warn!(%agent_id, error = %e, "ws proxy closed with error");
        }
        audit::log(
            &db,
            Some(org_id),
            Some(agent_id),
            Some(user_id),
            "agent.proxy.ws.closed",
            None,
        )
        .await;
    }))
}

async fn bridge(client: WebSocket, engine_url: &str, token: &str) -> anyhow::Result<()> {
    // `into_client_request` generates the mandatory handshake headers
    // (Sec-WebSocket-Key, Upgrade, Connection, Version, Host). Building a bare
    // `Request` by hand omits them and tungstenite rejects the handshake.
    //
    // Auth rides the `Authorization` header (engine reads it first). We do NOT
    // request a `Sec-WebSocket-Protocol`: the engine upgrade doesn't echo one,
    // and an unconfirmed requested subprotocol makes tungstenite fail the
    // handshake with "Server sent no subprotocol".
    let mut req = engine_url.into_client_request()?;
    req.headers_mut()
        .insert("Authorization", format!("Bearer {token}").parse()?);
    let (engine, _) = connect_async(req).await?;
    let (mut engine_tx, mut engine_rx) = engine.split();
    let (mut client_tx, mut client_rx) = client.split();
    let client_to_engine = async {
        while let Some(Ok(msg)) = client_rx.next().await {
            let out = match msg {
                Message::Text(t) => tokio_tungstenite::tungstenite::Message::Text(t),
                Message::Binary(b) => tokio_tungstenite::tungstenite::Message::Binary(b),
                Message::Ping(p) => tokio_tungstenite::tungstenite::Message::Ping(p),
                Message::Pong(p) => tokio_tungstenite::tungstenite::Message::Pong(p),
                Message::Close(c) => {
                    let out = c.map(|f| tokio_tungstenite::tungstenite::protocol::CloseFrame {
                        code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::from(
                            f.code,
                        ),
                        reason: f.reason,
                    });
                    let _ = engine_tx
                        .send(tokio_tungstenite::tungstenite::Message::Close(out))
                        .await;
                    break;
                }
            };
            engine_tx.send(out).await?;
        }
        Ok::<(), anyhow::Error>(())
    };
    let engine_to_client = async {
        while let Some(Ok(msg)) = engine_rx.next().await {
            let out = match msg {
                tokio_tungstenite::tungstenite::Message::Text(t) => Message::Text(t),
                tokio_tungstenite::tungstenite::Message::Binary(b) => Message::Binary(b),
                tokio_tungstenite::tungstenite::Message::Ping(p) => Message::Ping(p),
                tokio_tungstenite::tungstenite::Message::Pong(p) => Message::Pong(p),
                tokio_tungstenite::tungstenite::Message::Close(c) => {
                    let out = c.map(|f| axum::extract::ws::CloseFrame {
                        code: f.code.into(),
                        reason: f.reason,
                    });
                    let _ = client_tx.send(Message::Close(out)).await;
                    break;
                }
                _ => continue,
            };
            client_tx.send(out).await?;
        }
        Ok::<(), anyhow::Error>(())
    };
    tokio::select! {
        r = client_to_engine => r?,
        r = engine_to_client => r?,
    }
    Ok(())
}

async fn load_runtime(db: &Db, agent_id: Uuid) -> ApiResult<RuntimeTarget> {
    sqlx::query_as::<_, RuntimeTarget>(
        "SELECT internal_url, engine_token FROM cloud_agent_runtimes WHERE agent_id = $1",
    )
    .bind(agent_id)
    .fetch_optional(db.pool())
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?
    .ok_or_else(|| ApiError::not_found("Runtime not provisioned"))
}
