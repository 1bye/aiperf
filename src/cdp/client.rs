use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

pub struct CdpClient {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    next_id: u64,
}

pub enum CdpMessage {
    Response {
        id: u64,
        result: Option<Value>,
        error: Option<Value>,
    },
    Event {
        method: String,
        params: Value,
    },
}

impl CdpClient {
    pub async fn connect(ws_url: &str) -> anyhow::Result<Self> {
        let (ws, _) = connect_async(ws_url)
            .await
            .map_err(|e| anyhow::anyhow!("connect CDP websocket {ws_url}: {e}"))?;
        Ok(Self { ws, next_id: 1 })
    }
    pub async fn send(&mut self, method: &str, params: Value) -> anyhow::Result<u64> {
        let id = self.next_id;
        self.next_id += 1;
        let msg = json!({"id": id, "method": method, "params": params});
        self.ws.send(Message::Text(msg.to_string().into())).await?;
        Ok(id)
    }
    pub async fn recv(&mut self) -> anyhow::Result<Option<CdpMessage>> {
        while let Some(msg) = self.ws.next().await {
            let msg = msg?;
            let text = match msg {
                Message::Text(t) => t.to_string(),
                Message::Binary(b) => String::from_utf8_lossy(&b).to_string(),
                Message::Close(_) => return Ok(None),
                _ => continue,
            };
            let v: Value = serde_json::from_str(&text)?;
            if let Some(id) = v.get("id").and_then(Value::as_u64) {
                return Ok(Some(CdpMessage::Response {
                    id,
                    result: v.get("result").cloned(),
                    error: v.get("error").cloned(),
                }));
            }
            if let Some(method) = v.get("method").and_then(Value::as_str) {
                return Ok(Some(CdpMessage::Event {
                    method: method.to_string(),
                    params: v.get("params").cloned().unwrap_or(Value::Null),
                }));
            }
        }
        Ok(None)
    }
    pub async fn call_collecting_events<F>(
        &mut self,
        method: &str,
        params: Value,
        mut on_event: F,
    ) -> anyhow::Result<Value>
    where
        F: FnMut(&str, &Value) -> anyhow::Result<()>,
    {
        let id = self.send(method, params).await?;
        loop {
            match self.recv().await? {
                Some(CdpMessage::Response {
                    id: rid,
                    result,
                    error,
                }) if rid == id => {
                    if let Some(e) = error {
                        return Err(anyhow::anyhow!("CDP {method} error: {e}"));
                    }
                    return Ok(result.unwrap_or(Value::Null));
                }
                Some(CdpMessage::Event { method, params }) => on_event(&method, &params)?,
                Some(_) => {}
                None => {
                    return Err(anyhow::anyhow!(
                        "CDP connection closed while waiting for {method}"
                    ));
                }
            }
        }
    }
}
