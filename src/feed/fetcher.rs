use anyhow::Result;
use reqwest::Client;

pub async fn fetch_feed(client: &Client, url: &str) -> Result<String> {
    let resp = client
        .get(url)
        .header("User-Agent", "Nuzzle/0.1 (AI-native RSS reader)")
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("HTTP {} fetching {}", status, url);
    }

    let text = resp.text().await?;
    Ok(text)
}
