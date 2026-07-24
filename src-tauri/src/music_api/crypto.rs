use aes::cipher::{generic_array::GenericArray, BlockEncrypt, KeyInit};
use aes::Aes128;
use md5::{Digest, Md5};

/// 网易云 EAPI 加密密钥
const NETEASE_EAPI_KEY: &[u8; 16] = b"e82ckenh8dichen8";

/// 计算 MD5 哈希
pub fn md5_bytes(input: &[u8]) -> [u8; 16] {
    let mut hasher = Md5::new();
    hasher.update(input);
    hasher.finalize().into()
}

/// AES-128-ECB 加密 (PKCS7 填充)
pub fn aes_ecb_encrypt_pkcs7(input: &[u8], key: &[u8; 16]) -> Vec<u8> {
    let cipher = Aes128::new(GenericArray::from_slice(key));
    let mut buffer = input.to_vec();
    let pad_len = 16 - (buffer.len() % 16);
    buffer.extend(std::iter::repeat(pad_len as u8).take(pad_len));

    for chunk in buffer.chunks_exact_mut(16) {
        cipher.encrypt_block(GenericArray::from_mut_slice(chunk));
    }

    buffer
}

/// 网易云 EAPI 加密
/// 1. 计算 digest = MD5("nobody{api_path}use{payload}md5forencrypt")
/// 2. 拼接 data = "{api_path}-36cd479b6b5-{payload}-36cd479b6b5-{digest}"
/// 3. AES-128-ECB 加密 data
/// 4. 十六进制大写编码
pub fn encrypt_eapi_payload(api_path: &str, payload_text: &str) -> Result<String, String> {
    let digest_source = format!("nobody{api_path}use{payload_text}md5forencrypt");
    let digest = hex::encode(md5_bytes(digest_source.as_bytes()));
    let data = format!("{api_path}-36cd479b6b5-{payload_text}-36cd479b6b5-{digest}");
    let encrypted = aes_ecb_encrypt_pkcs7(data.as_bytes(), NETEASE_EAPI_KEY);
    Ok(hex::encode_upper(encrypted))
}

/// 构建 EAPI 请求头 (作为 Cookie 发送)
pub fn build_eapi_header() -> serde_json::Map<String, serde_json::Value> {
    use serde_json::json;
    let now_ms = chrono::Utc::now().timestamp_millis();
    let request_id = format!("{now_ms}_{:04}", rand::random::<u32>() % 1000);

    let mut map = serde_json::Map::new();
    map.insert("__csrf".into(), json!(""));
    map.insert("appver".into(), json!("8.0.0"));
    map.insert("buildver".into(), json!(now_ms / 1000));
    map.insert("channel".into(), json!(""));
    map.insert("deviceId".into(), json!(""));
    map.insert("mobilename".into(), json!(""));
    map.insert("resolution".into(), json!("1920x1080"));
    map.insert("os".into(), json!("android"));
    map.insert("osver".into(), json!(""));
    map.insert("requestId".into(), json!(request_id));
    map.insert("versioncode".into(), json!("140"));
    map.insert("MUSIC_U".into(), json!(""));
    map
}
