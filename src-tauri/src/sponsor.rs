use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct SponsorRaw {
    pub update_time: String,
    pub list: Vec<SponsorRawItem>,
}

#[derive(Debug, Deserialize)]
struct SponsorRawItem {
    pub name: String,
    pub amount: String,
}

#[derive(Debug, Serialize)]
pub struct SponsorRoot {
    pub update_time: String,
    pub list: Vec<SponsorItem>,
    /// 累计赞助总金额（元）
    pub total_amount: String,
}

#[derive(Debug, Serialize)]
pub struct SponsorItem {
    pub name: String,
    pub amount: String,
}

/// 从 "10元"、"6.66元" 等字符串中解析出数值（元）
fn parse_amount(s: &str) -> f64 {
    let digits: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    digits.parse::<f64>().unwrap_or(0.0)
}

/// 将金额格式化为去掉多余小数的字符串
fn format_number(v: f64) -> String {
    let rounded = (v * 100.0).round() / 100.0;
    if rounded.fract() == 0.0 {
        format!("{}", rounded as i64)
    } else {
        format!("{}", rounded)
    }
}

async fn fetch_sponsors() -> Result<SponsorRoot, reqwest::Error> {
    let url = "https://gitee.com/muliuawa/nexbox/raw/master/sponsors.json";

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let resp = client.get(url).send().await?;
    let data = resp.json::<SponsorRaw>().await?;

    // 全体累计总额
    let grand_total: f64 = data.list.iter().map(|item| parse_amount(&item.amount)).sum();

    let list = data
        .list
        .into_iter()
        .map(|item| SponsorItem {
            name: item.name,
            amount: item.amount,
        })
        .collect();

    Ok(SponsorRoot {
        update_time: data.update_time,
        list,
        total_amount: format_number(grand_total),
    })
}

#[tauri::command]
pub async fn get_sponsors() -> Result<SponsorRoot, String> {
    fetch_sponsors().await.map_err(|e| e.to_string())
}
