use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct CdpTarget {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub target_type: Option<String>,
    pub title: Option<String>,
    pub url: Option<String>,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub websocket_debugger_url: Option<String>,
}

pub async fn resolve_ws(
    cdp: &str,
    url_substr: Option<&str>,
    title_substr: Option<&str>,
) -> anyhow::Result<String> {
    if cdp.starts_with("ws://") || cdp.starts_with("wss://") {
        return Ok(cdp.to_string());
    }
    let endpoint = format!("{}/json/list", cdp.trim_end_matches('/'));
    let targets: Vec<CdpTarget> = reqwest::get(&endpoint)
        .await?
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("read CDP targets from {endpoint}: {e}"))?;
    let selected = targets
        .iter()
        .filter(|t| t.target_type.as_deref() == Some("page"))
        .find(|t| {
            url_substr.is_some_and(|s| t.url.as_deref().is_some_and(|u| u.contains(s)))
                || title_substr.is_some_and(|s| t.title.as_deref().is_some_and(|u| u.contains(s)))
        })
        .or_else(|| {
            targets
                .iter()
                .find(|t| t.target_type.as_deref() == Some("page"))
        })
        .or_else(|| targets.first())
        .ok_or_else(|| anyhow::anyhow!("no CDP targets from {endpoint}"))?;
    selected.websocket_debugger_url.clone().ok_or_else(|| {
        anyhow::anyhow!(
            "selected CDP target {:?} lacks webSocketDebuggerUrl",
            selected.id
        )
    })
}
