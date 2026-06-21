use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Deserialize, Serialize)]
pub struct SponsorRoot {
    pub update_time: String,
    pub list: Vec<SponsorItem>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SponsorItem {
    pub name: String,
    pub amount: String,
}

async fn fetch_sponsors() -> Result<SponsorRoot, reqwest::Error> {
    let url = "https://gitee.com/muliuawa/nexbox/raw/master/sponsors.json";

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let resp = client.get(url).send().await?;
    let data = resp.json::<SponsorRoot>().await?;
    Ok(data)
}

#[tauri::command]
pub async fn get_sponsors() -> Result<SponsorRoot, String> {
    fetch_sponsors().await.map_err(|e| e.to_string())
}
