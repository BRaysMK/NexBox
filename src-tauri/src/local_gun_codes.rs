//! 本地改枪码存储模块（三角洲行动）
//!
//! 功能：
//! - 本地持久化改枪码（枪名 / 改枪码 / 配置备注）
//! - 粘贴代码时自动识别枪名 + 配置名（备注）

use std::sync::Mutex;
use tauri_plugin_store::StoreExt;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LocalGunCode {
    pub id: String,
    pub weapon_name: String,
    pub code: String,
    pub note: String,
    pub created_at: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ImportResult {
    pub weapon_name: String,
    pub note: String,
    pub recognized: bool,
}

/// 内存锁，防止并发读写
static STORE_LOCK: Mutex<()> = Mutex::new(());

/// 获取存储路径
fn store_path() -> &'static str {
    "local_gun_codes.json"
}

/// 读取全部本地改枪码
pub fn load_all(app: &tauri::AppHandle) -> Vec<LocalGunCode> {
    let _guard = STORE_LOCK.lock().unwrap();
    match app.store(store_path()) {
        Ok(store) => {
            if let Some(value) = store.get("items") {
                if let Ok(items) = serde_json::from_value::<Vec<LocalGunCode>>(value) {
                    return items;
                }
            }
        }
        Err(e) => log::warn!("[LocalGunCode] 打开存储失败: {e}"),
    }
    Vec::new()
}

/// 保存全部改枪码
fn save_all(app: &tauri::AppHandle, items: &[LocalGunCode]) {
    let _guard = STORE_LOCK.lock().unwrap();
    match app.store(store_path()) {
        Ok(store) => {
            store.set("items", serde_json::to_value(items).unwrap());
            if let Err(e) = store.save() {
                log::error!("[LocalGunCode] 保存失败: {e}");
            }
        }
        Err(e) => log::error!("[LocalGunCode] 打开存储失败: {e}"),
    }
}

/// 前缀映射存储 key（{前缀: (枪名, 配置名)}）
fn prefix_map_key() -> &'static str {
    "prefix_map"
}

/// 读取前缀映射（兼容旧版仅存枪名的格式）
fn load_prefix_map(app: &tauri::AppHandle) -> std::collections::HashMap<String, (String, String)> {
    let _guard = STORE_LOCK.lock().unwrap();
    match app.store(store_path()) {
        Ok(store) => {
            if let Some(value) = store.get(prefix_map_key()) {
                // 新格式：[枪名, 配置名]
                if let Ok(map) = serde_json::from_value::<std::collections::HashMap<String, (String, String)>>(value.clone()) {
                    return map;
                }
                // 旧格式：{前缀: 枪名}
                if let Ok(map) = serde_json::from_value::<std::collections::HashMap<String, String>>(value) {
                    return map.into_iter().map(|(k, v)| (k, (v, String::new()))).collect();
                }
            }
        }
        Err(e) => log::warn!("[LocalGunCode] 读取前缀映射失败: {e}"),
    }
    std::collections::HashMap::new()
}

/// 保存前缀映射
fn save_prefix_map(app: &tauri::AppHandle, map: &std::collections::HashMap<String, (String, String)>) {
    let _guard = STORE_LOCK.lock().unwrap();
    match app.store(store_path()) {
        Ok(store) => {
            store.set(prefix_map_key(), serde_json::to_value(map).unwrap());
            if let Err(e) = store.save() {
                log::error!("[LocalGunCode] 保存前缀映射失败: {e}");
            }
        }
        Err(e) => log::error!("[LocalGunCode] 保存前缀映射失败: {e}"),
    }
}

/// 内置种子映射（前 6 位代码前缀 → (枪名, 配置名)）
///
/// 实测：三角洲改枪码前 6 位前缀在 244 条样本（63 把枪）中 100% 唯一（前 4/5 位会冲突）。
fn builtin_prefix_map() -> std::collections::HashMap<String, (String, String)> {
    let mut m = std::collections::HashMap::new();
    // ── 725 ──
    m.insert("6JO0LT".to_string(), ("725".to_string(), "8W腰射喷".to_string()));
    m.insert("6JO0LV".to_string(), ("725".to_string(), "15W开镜莽侠".to_string()));
    // ── 93R ──
    m.insert("6JO0M2".to_string(), ("93R".to_string(), "15W-烽火地带".to_string()));
    // ── 巴雷特 ──
    m.insert("6KGA77".to_string(), ("巴雷特".to_string(), "55W开镜最快".to_string()));
    m.insert("6KGA7F".to_string(), ("巴雷特".to_string(), "61W初速最快".to_string()));
    // ── 杠杆步枪 ──
    m.insert("6JO0G6".to_string(), ("杠杆步枪".to_string(), "33W开镜".to_string()));
    m.insert("6JO0G7".to_string(), ("杠杆步枪".to_string(), "15W开镜".to_string()));
    m.insert("6JO0GA".to_string(), ("杠杆步枪".to_string(), "16W腰射".to_string()));
    // ── 弓 ──
    m.insert("6KFCNR".to_string(), ("弓".to_string(), "20W速射腰射".to_string()));
    m.insert("6KFCO6".to_string(), ("弓".to_string(), "24W速射开镜".to_string()));
    m.insert("6KFCOD".to_string(), ("弓".to_string(), "48W开镜".to_string()));
    m.insert("6KFCOK".to_string(), ("弓".to_string(), "44W腰射弓".to_string()));
    // ── 沙鹰 ──
    m.insert("6JO0M7".to_string(), ("沙鹰".to_string(), "7W青春版".to_string()));
    m.insert("6JO0MF".to_string(), ("沙鹰".to_string(), "17W满改".to_string()));
    // ── 腾龙 ──
    m.insert("6JD8IU".to_string(), ("腾龙".to_string(), "".to_string()));
    m.insert("6JDAO8".to_string(), ("腾龙".to_string(), "".to_string()));
    m.insert("6JDJ8V".to_string(), ("腾龙".to_string(), "".to_string()));
    m.insert("6JNVDP".to_string(), ("腾龙".to_string(), "57W腰射版".to_string()));
    m.insert("6JNVDR".to_string(), ("腾龙".to_string(), "26W青春版".to_string()));
    m.insert("6JNVE2".to_string(), ("腾龙".to_string(), "50W稳固三倍".to_string()));
    m.insert("6KFCJM".to_string(), ("腾龙".to_string(), "50W高速两倍".to_string()));
    m.insert("6KFCKF".to_string(), ("腾龙".to_string(), "33W半改高速".to_string()));
    // ── 野牛 ──
    m.insert("6JNVQC".to_string(), ("野牛".to_string(), "7W".to_string()));
    m.insert("6JNVQS".to_string(), ("野牛".to_string(), "55W满改带爆闪".to_string()));
    // ── 勇士 ──
    m.insert("6JNVOB".to_string(), ("勇士".to_string(), "23W青春版腰射".to_string()));
    m.insert("6JNVOE".to_string(), ("勇士".to_string(), "14W青春版开镜".to_string()));
    m.insert("6JNVOP".to_string(), ("勇士".to_string(), "53W满改腰射".to_string()));
    // ── 左轮 ──
    m.insert("6JO0NG".to_string(), ("左轮".to_string(), "30W三倍狙".to_string()));
    // ── AK-12 ──
    m.insert("6JDHF8".to_string(), ("AK-12".to_string(), "".to_string()));
    m.insert("6JEQHH".to_string(), ("AK-12".to_string(), "".to_string()));
    m.insert("6JNVFO".to_string(), ("AK-12".to_string(), "17W青春版".to_string()));
    m.insert("6JNVFQ".to_string(), ("AK-12".to_string(), "55W满改腰射".to_string()));
    m.insert("6JNVG7".to_string(), ("AK-12".to_string(), "51W满改火控".to_string()));
    m.insert("6KFCL8".to_string(), ("AK-12".to_string(), "35W半改".to_string()));
    // ── AKM ──
    m.insert("6JNVLK".to_string(), ("AKM".to_string(), "20W青春版".to_string()));
    m.insert("6JNVLM".to_string(), ("AKM".to_string(), "25W腰射CS".to_string()));
    m.insert("6JNVLN".to_string(), ("AKM".to_string(), "38W半改".to_string()));
    m.insert("6KFCME".to_string(), ("AKM".to_string(), "53W满改二倍".to_string()));
    // ── AKS-74U ──
    m.insert("6JNVKB".to_string(), ("AKS-74U".to_string(), "10W青春版".to_string()));
    // ── AR57 ──
    m.insert("6JNV7P".to_string(), ("AR57".to_string(), "24W青春版".to_string()));
    m.insert("6JNV8G".to_string(), ("AR57".to_string(), "39W半改".to_string()));
    m.insert("6JNV94".to_string(), ("AR57".to_string(), "56W双修".to_string()));
    m.insert("6JNVA9".to_string(), ("AR57".to_string(), "63W满改两倍".to_string()));
    m.insert("6JNVAH".to_string(), ("AR57".to_string(), "66W满改红点".to_string()));
    m.insert("6JO6SI".to_string(), ("AR57".to_string(), "".to_string()));
    m.insert("6JO6U5".to_string(), ("AR57".to_string(), "".to_string()));
    m.insert("6JOFC1".to_string(), ("AR57".to_string(), "20W腰射100".to_string()));
    // ── AS Val ──
    m.insert("6JCKLL".to_string(), ("AS Val".to_string(), "".to_string()));
    m.insert("6JDHM8".to_string(), ("AS Val".to_string(), "".to_string()));
    m.insert("6JDHM9".to_string(), ("AS Val".to_string(), "".to_string()));
    m.insert("6JDHMB".to_string(), ("AS Val".to_string(), "".to_string()));
    m.insert("6JDHML".to_string(), ("AS Val".to_string(), "".to_string()));
    m.insert("6JNVE6".to_string(), ("AS Val".to_string(), "39W半改".to_string()));
    m.insert("6JNVEA".to_string(), ("AS Val".to_string(), "78W双修".to_string()));
    m.insert("6JNVEG".to_string(), ("AS Val".to_string(), "78W刺客四连发".to_string()));
    m.insert("6JNVEJ".to_string(), ("AS Val".to_string(), "75W满改红点".to_string()));
    m.insert("6JNVVM".to_string(), ("AS Val".to_string(), "".to_string()));
    m.insert("6KVLAQ".to_string(), ("AS Val".to_string(), "".to_string()));
    // ── ASH-12 ──
    m.insert("6JNVJT".to_string(), ("ASH-12".to_string(), "30W青春版".to_string()));
    m.insert("6JNVK0".to_string(), ("ASH-12".to_string(), "60W满改腰射".to_string()));
    m.insert("6JNVK6".to_string(), ("ASH-12".to_string(), "50W战斧二倍".to_string()));
    m.insert("6JNVK8".to_string(), ("ASH-12".to_string(), "50W长枪管二倍".to_string()));
    m.insert("6KFCT5".to_string(), ("ASH-12".to_string(), "金枪客".to_string()));
    m.insert("6KFSC9".to_string(), ("ASH-12".to_string(), "46W新枪管2/4".to_string()));
    // ── AUG ──
    m.insert("6JNVHG".to_string(), ("AUG".to_string(), "19W丐版三倍".to_string()));
    m.insert("6JNVHR".to_string(), ("AUG".to_string(), "23W青春版".to_string()));
    m.insert("6JNVI2".to_string(), ("AUG".to_string(), "31W半改".to_string()));
    m.insert("6JNVI7".to_string(), ("AUG".to_string(), "53W满改三倍".to_string()));
    // ── AWM ──
    m.insert("6JO08H".to_string(), ("AWM".to_string(), "96W初速快".to_string()));
    m.insert("6JO0F8".to_string(), ("AWM".to_string(), "真半改AW".to_string()));
    m.insert("6JO0FK".to_string(), ("AWM".to_string(), "82W开镜快".to_string()));
    // ── CAR-15 ──
    m.insert("6JNVEN".to_string(), ("CAR-15".to_string(), "10W青春版".to_string()));
    m.insert("6JNVEO".to_string(), ("CAR-15".to_string(), "13W腰射版".to_string()));
    // ── FS-12 ──
    m.insert("6JO0NS".to_string(), ("FS-12".to_string(), "31W".to_string()));
    // ── G18 ──
    m.insert("6JO0M4".to_string(), ("G18".to_string(), "12W".to_string()));
    // ── G3 ──
    m.insert("6JDJCQ".to_string(), ("G3".to_string(), "".to_string()));
    m.insert("6JDJD5".to_string(), ("G3".to_string(), "".to_string()));
    m.insert("6JNV6T".to_string(), ("G3".to_string(), "10W青春版".to_string()));
    m.insert("6JNV6V".to_string(), ("G3".to_string(), "28W半改".to_string()));
    m.insert("6JNV72".to_string(), ("G3".to_string(), "43W满改三倍".to_string()));
    // ── K416 ──
    m.insert("6JNVIQ".to_string(), ("K416".to_string(), "22W青春版".to_string()));
    m.insert("6JNVIT".to_string(), ("K416".to_string(), "53W腰射".to_string()));
    m.insert("6JNVJP".to_string(), ("K416".to_string(), "66W满改火控".to_string()));
    m.insert("6JUDCB".to_string(), ("K416".to_string(), "48W满改红点".to_string()));
    m.insert("6K2GRV".to_string(), ("K416".to_string(), "35W半改".to_string()));
    // ── K437 ──
    m.insert("6JDGAP".to_string(), ("K437".to_string(), "".to_string()));
    m.insert("6JDGAQ".to_string(), ("K437".to_string(), "".to_string()));
    m.insert("6JDGAR".to_string(), ("K437".to_string(), "".to_string()));
    m.insert("6JDGAS".to_string(), ("K437".to_string(), "".to_string()));
    m.insert("6JNVD8".to_string(), ("K437".to_string(), "23W青春版".to_string()));
    m.insert("6JNVDB".to_string(), ("K437".to_string(), "36W半改".to_string()));
    m.insert("6JNVDE".to_string(), ("K437".to_string(), "75W满改红点".to_string()));
    m.insert("6JNVDG".to_string(), ("K437".to_string(), "65W满改火控".to_string()));
    // ── KC17 ──
    m.insert("6JCB3D".to_string(), ("KC17".to_string(), "".to_string()));
    m.insert("6JDJ9N".to_string(), ("KC17".to_string(), "".to_string()));
    m.insert("6JDJAI".to_string(), ("KC17".to_string(), "".to_string()));
    m.insert("6JDJB1".to_string(), ("KC17".to_string(), "".to_string()));
    m.insert("6JNVCR".to_string(), ("KC17".to_string(), "25W青春版".to_string()));
    m.insert("6JNVD0".to_string(), ("KC17".to_string(), "40W半改".to_string()));
    m.insert("6JNVD1".to_string(), ("KC17".to_string(), "76W满改火控".to_string()));
    m.insert("6JOPRU".to_string(), ("KC17".to_string(), "72W红点".to_string()));
    // ── M1014 ──
    m.insert("6JO0LQ".to_string(), ("M1014".to_string(), "32W".to_string()));
    // ── M14 ──
    m.insert("6JO02S".to_string(), ("M14".to_string(), "满改腰射".to_string()));
    m.insert("6JO03M".to_string(), ("M14".to_string(), "31W青春版".to_string()));
    m.insert("6JO03N".to_string(), ("M14".to_string(), "42W半改".to_string()));
    m.insert("6JO03R".to_string(), ("M14".to_string(), "44W半改红点".to_string()));
    m.insert("6JO04A".to_string(), ("M14".to_string(), "75W满改红点".to_string()));
    m.insert("6JO04J".to_string(), ("M14".to_string(), "67W满改三倍".to_string()));
    m.insert("6JO04P".to_string(), ("M14".to_string(), "76W满改消音".to_string()));
    // ── M16A4 ──
    m.insert("6JNVII".to_string(), ("M16A4".to_string(), "48W三倍".to_string()));
    m.insert("6JNVIL".to_string(), ("M16A4".to_string(), "27W腰射三连发".to_string()));
    // ── M1911 ──
    m.insert("6JO1LU".to_string(), ("M1911".to_string(), "12W".to_string()));
    // ── M249 ──
    m.insert("6JO0JI".to_string(), ("M249".to_string(), "22W青春版".to_string()));
    m.insert("6JO0JP".to_string(), ("M249".to_string(), "39W满改两倍".to_string()));
    m.insert("6JPPL0".to_string(), ("M249".to_string(), "37W链锯腰射".to_string()));
    // ── M250 ──
    m.insert("6JO0GI".to_string(), ("M250".to_string(), "30W青春版".to_string()));
    m.insert("6JO0GK".to_string(), ("M250".to_string(), "45W半改".to_string()));
    m.insert("6JO0GM".to_string(), ("M250".to_string(), "85W满改三倍".to_string()));
    m.insert("6JO0GP".to_string(), ("M250".to_string(), "77W低倍".to_string()));
    // ── M4A1 ──
    m.insert("6JNVLR".to_string(), ("M4A1".to_string(), "19W青春版".to_string()));
    m.insert("6JNVLS".to_string(), ("M4A1".to_string(), "25W半改腰射".to_string()));
    m.insert("6JNVLT".to_string(), ("M4A1".to_string(), "36W半改".to_string()));
    m.insert("6JNVLV".to_string(), ("M4A1".to_string(), "59W最强腰射".to_string()));
    m.insert("6JNVM7".to_string(), ("M4A1".to_string(), "59W满改三倍".to_string()));
    m.insert("6JNVMB".to_string(), ("M4A1".to_string(), "65W红点".to_string()));
    // ── M7 ──
    m.insert("6J8NL4".to_string(), ("M7".to_string(), "".to_string()));
    m.insert("6JCB21".to_string(), ("M7".to_string(), "".to_string()));
    m.insert("6JEU7L".to_string(), ("M7".to_string(), "".to_string()));
    m.insert("6JFFG4".to_string(), ("M7".to_string(), "".to_string()));
    m.insert("6JFGP0".to_string(), ("M7".to_string(), "".to_string()));
    m.insert("6JNVGS".to_string(), ("M7".to_string(), "39W青春版".to_string()));
    m.insert("6JNVH3".to_string(), ("M7".to_string(), "59W半改".to_string()));
    m.insert("6JNVH7".to_string(), ("M7".to_string(), "85W满改红点".to_string()));
    m.insert("6JO02U".to_string(), ("M7".to_string(), "满改腰射".to_string()));
    m.insert("6K3VAH".to_string(), ("M7".to_string(), "".to_string()));
    m.insert("6K3VBM".to_string(), ("M7".to_string(), "".to_string()));
    m.insert("6KAO6G".to_string(), ("M7".to_string(), "战斗步枪-烽火地带".to_string()));
    m.insert("6KFHP6".to_string(), ("M7".to_string(), "85W满改3.5倍".to_string()));
    // ── M700 ──
    m.insert("6JO04U".to_string(), ("M700".to_string(), "29W".to_string()));
    m.insert("6JO05B".to_string(), ("M700".to_string(), "46W秒开镜".to_string()));
    m.insert("6JO05E".to_string(), ("M700".to_string(), "54W初速快".to_string()));
    // ── M870 ──
    m.insert("6JO1N1".to_string(), ("M870".to_string(), "38W".to_string()));
    // ── MCX ──
    m.insert("6JDGAU".to_string(), ("MCX".to_string(), "".to_string()));
    m.insert("6JI40H".to_string(), ("MCX".to_string(), "".to_string()));
    m.insert("6JNVAS".to_string(), ("MCX".to_string(), "25W长管青春版".to_string()));
    m.insert("6JNVAU".to_string(), ("MCX".to_string(), "29W短管青春版".to_string()));
    m.insert("6JNVB9".to_string(), ("MCX".to_string(), "50W无枪管腰射".to_string()));
    m.insert("6JNVBH".to_string(), ("MCX".to_string(), "54W短管满改".to_string()));
    m.insert("6JNVBR".to_string(), ("MCX".to_string(), "55W长管火控".to_string()));
    m.insert("6KS5RF".to_string(), ("MCX".to_string(), "39W短管半改".to_string()));
    // ── MINI14 ──
    m.insert("6JO06V".to_string(), ("MINI14".to_string(), "24W半改".to_string()));
    m.insert("6JO074".to_string(), ("MINI14".to_string(), "51W满改".to_string()));
    // ── MK4 ──
    m.insert("6JNVS7".to_string(), ("MK4".to_string(), "19W青春版".to_string()));
    m.insert("6JNVS9".to_string(), ("MK4".to_string(), "57W全自动全能".to_string()));
    m.insert("6JNVSB".to_string(), ("MK4".to_string(), "54W全能三连发".to_string()));
    m.insert("6JNVSH".to_string(), ("MK4".to_string(), "66W全自动红点".to_string()));
    m.insert("6JNVSU".to_string(), ("MK4".to_string(), "31W全自动半改".to_string()));
    // ── MK47 ──
    m.insert("6IVP15".to_string(), ("MK47".to_string(), "".to_string()));
    m.insert("6JB8RH".to_string(), ("MK47".to_string(), "".to_string()));
    m.insert("6JDJBL".to_string(), ("MK47".to_string(), "".to_string()));
    m.insert("6JDJC7".to_string(), ("MK47".to_string(), "".to_string()));
    m.insert("6JNVC5".to_string(), ("MK47".to_string(), "31W青春版".to_string()));
    m.insert("6JNVC7".to_string(), ("MK47".to_string(), "54W半改".to_string()));
    m.insert("6JNVCA".to_string(), ("MK47".to_string(), "77W纯腰射".to_string()));
    m.insert("6JNVCC".to_string(), ("MK47".to_string(), "77W全能版".to_string()));
    m.insert("6JNVCE".to_string(), ("MK47".to_string(), "89W满改二倍".to_string()));
    m.insert("6JNVCG".to_string(), ("MK47".to_string(), "75W满改红点".to_string()));
    m.insert("6JVB7N".to_string(), ("MK47".to_string(), "".to_string()));
    m.insert("6K1AAC".to_string(), ("MK47".to_string(), "".to_string()));
    // ── MP5 ──
    m.insert("6JNVS1".to_string(), ("MP5".to_string(), "10W青春版腰射".to_string()));
    m.insert("6JNVS4".to_string(), ("MP5".to_string(), "44W满改腰射".to_string()));
    // ── MP7 ──
    m.insert("6JNVNH".to_string(), ("MP7".to_string(), "20W青春版腰射".to_string()));
    // ── P90 ──
    m.insert("6JNVRM".to_string(), ("P90".to_string(), "15W青春版".to_string()));
    m.insert("6JNVRP".to_string(), ("P90".to_string(), "38W新枪管".to_string()));
    m.insert("6JNVRV".to_string(), ("P90".to_string(), "42W满改".to_string()));
    // ── PKM ──
    m.insert("6JO0GS".to_string(), ("PKM".to_string(), "22W青春版".to_string()));
    m.insert("6JO0H1".to_string(), ("PKM".to_string(), "31W半改".to_string()));
    m.insert("6JO0H8".to_string(), ("PKM".to_string(), "57W满改2/4".to_string()));
    m.insert("6JO0HC".to_string(), ("PKM".to_string(), "60W满改三倍".to_string()));
    m.insert("6JO0HG".to_string(), ("PKM".to_string(), "59W满改红点".to_string()));
    // ── PSG-1 ──
    m.insert("6JO062".to_string(), ("PSG-1".to_string(), "31W青春版".to_string()));
    m.insert("6JO06C".to_string(), ("PSG-1".to_string(), "56W满改".to_string()));
    // ── PTR-32 ──
    m.insert("6JNVEQ".to_string(), ("PTR-32".to_string(), "12W青春版".to_string()));
    m.insert("6JNVEV".to_string(), ("PTR-32".to_string(), "27W半改".to_string()));
    // ── QBZ-95 ──
    m.insert("6JNVKD".to_string(), ("QBZ-95".to_string(), "17W青春版".to_string()));
    m.insert("6JNVKF".to_string(), ("QBZ-95".to_string(), "36W满改".to_string()));
    // ── QCQ171 ──
    m.insert("6JNVMK".to_string(), ("QCQ171".to_string(), "28W青春版".to_string()));
    m.insert("6JNVMO".to_string(), ("QCQ171".to_string(), "63W满改红点".to_string()));
    // ── QJB ──
    m.insert("6JO0HK".to_string(), ("QJB".to_string(), "20W青春版".to_string()));
    m.insert("6JO0HM".to_string(), ("QJB".to_string(), "44W满改腰射".to_string()));
    m.insert("6JO0J6".to_string(), ("QJB".to_string(), "58W全能版".to_string()));
    m.insert("6JO0J8".to_string(), ("QJB".to_string(), "56W红点高速".to_string()));
    m.insert("6JO0JB".to_string(), ("QJB".to_string(), "46W100稳固".to_string()));
    // ── R93 ──
    m.insert("6JO086".to_string(), ("R93".to_string(), "22W".to_string()));
    // ── RM277 ──
    m.insert("6KFCC1".to_string(), ("RM277".to_string(), "20W青春版".to_string()));
    m.insert("6KFCCL".to_string(), ("RM277".to_string(), "38W半改".to_string()));
    m.insert("6KFCDO".to_string(), ("RM277".to_string(), "52W满改3.5倍".to_string()));
    m.insert("6KFCDT".to_string(), ("RM277".to_string(), "55W满改红点".to_string()));
    m.insert("6KFCQQ".to_string(), ("RM277".to_string(), "金枪客".to_string()));
    m.insert("6KFHOH".to_string(), ("RM277".to_string(), "".to_string()));
    m.insert("6KGHNR".to_string(), ("RM277".to_string(), "".to_string()));
    // ── S12K ──
    m.insert("6JO0LM".to_string(), ("S12K".to_string(), "32W撞火全自动腰射".to_string()));
    // ── SCAR-H ──
    m.insert("6JDJEU".to_string(), ("SCAR-H".to_string(), "".to_string()));
    m.insert("6JDJF6".to_string(), ("SCAR-H".to_string(), "".to_string()));
    m.insert("6JF88V".to_string(), ("SCAR-H".to_string(), "".to_string()));
    m.insert("6JFFED".to_string(), ("SCAR-H".to_string(), "".to_string()));
    m.insert("6JNVF7".to_string(), ("SCAR-H".to_string(), "19W青春版".to_string()));
    m.insert("6JNVF8".to_string(), ("SCAR-H".to_string(), "34W半改".to_string()));
    m.insert("6JNVFF".to_string(), ("SCAR-H".to_string(), "61W满改三倍".to_string()));
    m.insert("6JO03A".to_string(), ("SCAR-H".to_string(), "满改腰射".to_string()));
    // ── SG552 ──
    m.insert("6JFFET".to_string(), ("SG552".to_string(), "".to_string()));
    m.insert("6JFFF8".to_string(), ("SG552".to_string(), "".to_string()));
    m.insert("6JFFFJ".to_string(), ("SG552".to_string(), "".to_string()));
    m.insert("6JNVGB".to_string(), ("SG552".to_string(), "13W青春版".to_string()));
    m.insert("6JNVGI".to_string(), ("SG552".to_string(), "26W半改".to_string()));
    m.insert("6JNVGL".to_string(), ("SG552".to_string(), "62W满改2/4".to_string()));
    m.insert("6JNVGN".to_string(), ("SG552".to_string(), "64W满改红点".to_string()));
    m.insert("6JO033".to_string(), ("SG552".to_string(), "满改腰射".to_string()));
    // ── SKS ──
    m.insert("6JO0FS".to_string(), ("SKS".to_string(), "30W".to_string()));
    m.insert("6JO0G2".to_string(), ("SKS".to_string(), "59W".to_string()));
    // ── SMG45 ──
    m.insert("6JNV51".to_string(), ("SMG45".to_string(), "26W半改".to_string()));
    m.insert("6JNV5F".to_string(), ("SMG45".to_string(), "20W性价比腰射".to_string()));
    m.insert("6JNV5P".to_string(), ("SMG45".to_string(), "54W满改腰射".to_string()));
    m.insert("6JNV69".to_string(), ("SMG45".to_string(), "57W满改双修".to_string()));
    m.insert("6JNV6K".to_string(), ("SMG45".to_string(), "70W满改三倍".to_string()));
    m.insert("6JO1F6".to_string(), ("SMG45".to_string(), "12W青春版开镜".to_string()));
    // ── SR-3M ──
    m.insert("6JNVOU".to_string(), ("SR-3M".to_string(), "23W青春版".to_string()));
    m.insert("6JNVOV".to_string(), ("SR-3M".to_string(), "18W青春版腰射".to_string()));
    m.insert("6JNVP5".to_string(), ("SR-3M".to_string(), "40W半改全能".to_string()));
    m.insert("6JNVP7".to_string(), ("SR-3M".to_string(), "69W满改腰射".to_string()));
    m.insert("6JNVPH".to_string(), ("SR-3M".to_string(), "78W双休版".to_string()));
    m.insert("6JNVPK".to_string(), ("SR-3M".to_string(), "74W满改红点".to_string()));
    // ── SR25 ──
    m.insert("6JO07Q".to_string(), ("SR25".to_string(), "81W短管速射".to_string()));
    m.insert("6JO080".to_string(), ("SR25".to_string(), "91W满改".to_string()));
    // ── SV-98 ──
    m.insert("6JO08A".to_string(), ("SV-98".to_string(), "23W".to_string()));
    // ── SVCH ──
    m.insert("6KFCGH".to_string(), ("SVCH".to_string(), "55W满改红点".to_string()));
    m.insert("6KFCH1".to_string(), ("SVCH".to_string(), "58W满改2/4".to_string()));
    m.insert("6KFCSO".to_string(), ("SVCH".to_string(), "金枪客".to_string()));
    // ── SVD ──
    m.insert("6JO06L".to_string(), ("SVD".to_string(), "31W".to_string()));
    m.insert("6JO06Q".to_string(), ("SVD".to_string(), "60W".to_string()));
    // ── UZI ──
    m.insert("6JNVR3".to_string(), ("UZI".to_string(), "12W".to_string()));
    // ── Vector ──
    m.insert("6JNVRD".to_string(), ("Vector".to_string(), "57W腰射版".to_string()));
    m.insert("6JNVRI".to_string(), ("Vector".to_string(), "51W开镜".to_string()));
    // ── VSS ──
    m.insert("6JO07B".to_string(), ("VSS".to_string(), "66W".to_string()));

    m.insert("6JDBP7".to_string(), ("MK4".to_string(), "红点38w".to_string()));
    m.insert("6JKA32".to_string(), ("MK4".to_string(), "红点47w".to_string()));
    m.insert("6K97CM".to_string(), ("QCQ171".to_string(), "16W".to_string()));
    m.insert("6JG8D3".to_string(), ("QCQ171".to_string(), "28W半改".to_string()));
    m.insert("6JDBPB".to_string(), ("MP7".to_string(), "纯腰射24w".to_string()));
    m.insert("6ID9VE".to_string(), ("MP7".to_string(), "可腰可压48w".to_string()));
    m.insert("6HLS29".to_string(), ("SMG45".to_string(), "11w".to_string()));
    m.insert("6K6LJV".to_string(), ("SMG45".to_string(), "纯腰射15w".to_string()));
    m.insert("6JDBPC".to_string(), ("野牛".to_string(), "7W".to_string()));
    m.insert("6K6LK2".to_string(), ("UZI".to_string(), "13W".to_string()));
    m.insert("6JDADU".to_string(), ("Vector".to_string(), "纯腰射30W".to_string()));
    m.insert("6JKA3B".to_string(), ("Vector".to_string(), "可腰可压35w".to_string()));
    m.insert("6KR3K8".to_string(), ("P90".to_string(), "红点21W".to_string()));
    m.insert("6K7LE6".to_string(), ("P90".to_string(), "红点31W".to_string()));
    m.insert("6JS0QB".to_string(), ("MP5".to_string(), "10W".to_string()));
    m.insert("6K1ATM".to_string(), ("SR-3M".to_string(), "22w".to_string()));
    m.insert("6JCKMS".to_string(), ("SR-3M".to_string(), "红点26W".to_string()));
    m.insert("6IE8GH".to_string(), ("SR-3M".to_string(), "红点56w".to_string()));
    m.insert("6JKA36".to_string(), ("勇士".to_string(), "腰射9w".to_string()));
    m.insert("6KMPJV".to_string(), ("QJB".to_string(), "红点49w".to_string()));
    m.insert("6KMPK4".to_string(), ("QJB".to_string(), "倍镜44w".to_string()));
    m.insert("6KR3KB".to_string(), ("M250".to_string(), "33w".to_string()));
    m.insert("6KR3KC".to_string(), ("M250".to_string(), "42w无枪管".to_string()));
    m.insert("6K8SNB".to_string(), ("M250".to_string(), "红点63w".to_string()));
    m.insert("6KVF8J".to_string(), ("M250".to_string(), "63w苏醒同款".to_string()));
    m.insert("6K1ABM".to_string(), ("M250".to_string(), "3.5倍60w".to_string()));
    m.insert("6JCBMM".to_string(), ("PSG-1".to_string(), "36W".to_string()));
    m.insert("6JCBMO".to_string(), ("PSG-1".to_string(), "28w".to_string()));
    m.insert("6JF8EH".to_string(), ("SR25".to_string(), "48w".to_string()));
    m.insert("6JCLML".to_string(), ("SR25".to_string(), "61w".to_string()));
    m.insert("6KFHSK".to_string(), ("SVCH".to_string(), "54w".to_string()));
    m.insert("6KFHBI".to_string(), ("SVCH".to_string(), "48w倍镜".to_string()));
    m.insert("6JCBN3".to_string(), ("杠杆步枪".to_string(), "15w".to_string()));
    m.insert("6JCBN1".to_string(), ("杠杆步枪".to_string(), "23w".to_string()));
    m.insert("6JDAPT".to_string(), ("M14".to_string(), "51w".to_string()));
    m.insert("6KTGH7".to_string(), ("M14".to_string(), "3倍46w".to_string()));
    m.insert("6JSLAQ".to_string(), ("M14".to_string(), "半改37w".to_string()));
    m.insert("6JCBNI".to_string(), ("M14".to_string(), "红点51W".to_string()));
    m.insert("6K1AQ3".to_string(), ("M14".to_string(), "38w红点".to_string()));
    m.insert("6KR3KG".to_string(), ("M249".to_string(), "23W".to_string()));
    m.insert("6JCKL3".to_string(), ("M249".to_string(), "33w".to_string()));
    m.insert("6KR3KI".to_string(), ("M249".to_string(), "纯腰射36w".to_string()));
    m.insert("6JCKKV".to_string(), ("PKM".to_string(), "16w".to_string()));
    m.insert("6JDBP6".to_string(), ("PKM".to_string(), "红点29w".to_string()));
    m.insert("6JJG0B".to_string(), ("PKM".to_string(), "红点38W".to_string()));
    m.insert("6JJG0S".to_string(), ("PKM".to_string(), "倍41W".to_string()));
    m.insert("6JCBNL".to_string(), ("SVD".to_string(), "21w".to_string()));
    m.insert("6K40NR".to_string(), ("SVD".to_string(), "42w秒开镜".to_string()));
    m.insert("6JCBNR".to_string(), ("SVD".to_string(), "47W".to_string()));
    m.insert("6JCBNV".to_string(), ("VSS".to_string(), "37W".to_string()));
    m.insert("6JCBOD".to_string(), ("MINI14".to_string(), "17W".to_string()));
    m.insert("6JDAM3".to_string(), ("MINI14".to_string(), "27W".to_string()));
    m.insert("6JDAM4".to_string(), ("MINI14".to_string(), "31W".to_string()));
    m.insert("6JDAMA".to_string(), ("SKS".to_string(), "39w".to_string()));
    m.insert("6JFFGC".to_string(), ("AWM".to_string(), "87W".to_string()));
    m.insert("6JFFGC".to_string(), ("AWM".to_string(), "83W".to_string()));
    m.insert("6JDAN8".to_string(), ("M700".to_string(), "31W".to_string()));
    m.insert("6HHM76".to_string(), ("M700".to_string(), "36w".to_string()));
    m.insert("6JCBP3".to_string(), ("M700".to_string(), "42W".to_string()));
    m.insert("6JCBP8".to_string(), ("G18".to_string(), "12W".to_string()));
    m.insert("6JCBPB".to_string(), ("93R".to_string(), "13W".to_string()));
    m.insert("6JCBPE".to_string(), ("725".to_string(), "25W莽侠".to_string()));
    m.insert("6JDANN".to_string(), ("S12K".to_string(), "14w".to_string()));
    m.insert("6JDANP".to_string(), ("S12K".to_string(), "21w".to_string()));
    m.insert("6JCBPL".to_string(), ("弓".to_string(), "41w".to_string()));
    m.insert("6JCBPO".to_string(), ("弓".to_string(), "40w".to_string()));
    m.insert("6KFHUU".to_string(), ("弓".to_string(), "新套件25w".to_string()));
    m.insert("6JFFIU".to_string(), ("FS-12".to_string(), "23w".to_string()));
    m.insert("6JMF8G".to_string(), ("FS-12".to_string(), "31W".to_string()));
    m.insert("6JFRU2".to_string(), ("AUG".to_string(), "16W".to_string()));
    m.insert("6JDBPN".to_string(), ("AUG".to_string(), "红点46w".to_string()));
    m.insert("6JFRV4".to_string(), ("AUG".to_string(), "红点49w".to_string()));
    m.insert("6JFRUM".to_string(), ("AUG".to_string(), "倍镜46w".to_string()));
    m.insert("6K1AAS".to_string(), ("AUG".to_string(), "大倍镜46W".to_string()));
    m.insert("6JGRFE".to_string(), ("K416".to_string(), "18W".to_string()));
    m.insert("6JG5EO".to_string(), ("K416".to_string(), "30w红点".to_string()));
    m.insert("6JDAPF".to_string(), ("K416".to_string(), "红点45w".to_string()));
    m.insert("6HJCJD".to_string(), ("K416".to_string(), "3倍镜42w".to_string()));
    m.insert("6HK95B".to_string(), ("K416".to_string(), "红点44W".to_string()));
    m.insert("6JO73L".to_string(), ("ASH-12".to_string(), "激光25w".to_string()));
    m.insert("6JE5ED".to_string(), ("ASH-12".to_string(), "激光37w".to_string()));
    m.insert("6JDBPP".to_string(), ("ASH-12".to_string(), "倍镜36w".to_string()));
    m.insert("6KFHQM".to_string(), ("ASH-12".to_string(), "40w双发套件".to_string()));
    m.insert("6JS0NJ".to_string(), ("AKS-74U".to_string(), "10w".to_string()));
    m.insert("6JS0NQ".to_string(), ("QBZ-95".to_string(), "16W".to_string()));
    m.insert("6J4ME0".to_string(), ("QBZ-95".to_string(), "红点28w".to_string()));
    m.insert("6J4ME3".to_string(), ("QBZ-95".to_string(), "倍镜25w".to_string()));
    m.insert("6JS0Q6".to_string(), ("AKM".to_string(), "19w".to_string()));
    m.insert("6JG4BN".to_string(), ("AKM".to_string(), "倍镜24w".to_string()));
    m.insert("6KFHVM".to_string(), ("AKM".to_string(), "红点37w".to_string()));
    m.insert("6JUB0A".to_string(), ("AKM".to_string(), "纯腰射15w".to_string()));
    m.insert("6JUB0B".to_string(), ("M4A1".to_string(), "21w".to_string()));
    m.insert("6JUB0D".to_string(), ("M4A1".to_string(), "红点31w".to_string()));
    m.insert("6JUB0F".to_string(), ("M4A1".to_string(), "红点38w".to_string()));
    m.insert("6JUB0J".to_string(), ("M4A1".to_string(), "可腰可压34w".to_string()));
    m.insert("6K1AB4".to_string(), ("M4A1".to_string(), "倍镜34w".to_string()));
    m.insert("6JDBPQ".to_string(), ("CAR-15".to_string(), "9w".to_string()));
    m.insert("6K1AB8".to_string(), ("PTR-32".to_string(), "21W".to_string()));
    m.insert("6HN54R".to_string(), ("MK47".to_string(), "长枪管消音版".to_string()));
    m.insert("6HN54U".to_string(), ("MK47".to_string(), "长枪管二倍".to_string()));
    m.insert("6HN550".to_string(), ("MK47".to_string(), "长枪管高操速版".to_string()));
    m.insert("6HN553".to_string(), ("MK47".to_string(), "短枪管稳定腰射优秀".to_string()));
    m.insert("6GR2LT".to_string(), ("K437".to_string(), "7月3二倍消音50W".to_string()));
    m.insert("6GR2M2".to_string(), ("K437".to_string(), "红点满改55W".to_string()));
    m.insert("6GR2M3".to_string(), ("K437".to_string(), "性价比15W".to_string()));
    m.insert("6GR2M5".to_string(), ("K437".to_string(), "半满改44W".to_string()));
    m.insert("6HEURJ".to_string(), ("KC17".to_string(), "40W改装".to_string()));
    m.insert("6HPK5V".to_string(), ("M7".to_string(), "更新50W满改二倍".to_string()));
    m.insert("6I2MVE".to_string(), ("M7".to_string(), "自用".to_string()));
    m
}

/// 提取代码前缀（取前 6 位字母数字，转大写）
fn extract_prefix(code: &str) -> String {
    let clean: String = code.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    clean.chars().take(6).collect::<String>().to_uppercase()
}

/// 基于前缀映射识别枪名 + 配置名
fn recognize_by_prefix(app: &tauri::AppHandle, code: &str) -> Option<(String, String)> {
    let prefix = extract_prefix(code);
    if prefix.len() < 6 {
        return None;
    }

    // 合并：内置映射 + 学习映射（学习优先）
    let mut map = builtin_prefix_map();
    for (k, v) in load_prefix_map(app) {
        map.insert(k, v);
    }

    map.get(&prefix).cloned()
}

/// 学习：记住代码前缀对应的 (枪名, 配置名)
fn learn_prefix(app: &tauri::AppHandle, code: &str, weapon_name: &str, note: &str) {
    if weapon_name.trim().is_empty() {
        return;
    }
    let prefix = extract_prefix(code);
    if prefix.len() < 6 {
        return;
    }
    let mut map = load_prefix_map(app);
    map.insert(prefix.clone(), (weapon_name.trim().to_string(), note.trim().to_string()));
    save_prefix_map(app, &map);
    log::info!("[LocalGunCode] 已学习前缀 {prefix} → {weapon_name} / {note}");
}

/// 收集全部已知枪名（内置 + 学习），用于行内识别
fn known_gun_names(app: &tauri::AppHandle) -> std::collections::HashSet<String> {
    let mut known: std::collections::HashSet<String> = std::collections::HashSet::new();
    for v in builtin_prefix_map().values() {
        known.insert(v.0.clone());
    }
    for v in load_prefix_map(app).values() {
        known.insert(v.0.clone());
    }
    known
}

/// 行内识别（与批量导入同款逻辑）：一行文本中找 20~32 位代码，
/// 用「代码前」文本做已知枪名最长匹配（去连字符），配置名取代码后文本，
/// 其次取代码前去掉枪名后的文本。
fn recognize_from_line(app: &tauri::AppHandle, line: &str) -> Option<(String, String)> {
    let clean_all: String = line
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || c.is_whitespace())
        .collect();
    let mut code: Option<String> = None;
    for tok in clean_all.split_whitespace() {
        if tok.chars().all(|c| c.is_ascii_alphanumeric()) && tok.len() >= 20 && tok.len() <= 32 {
            code = Some(tok.to_string());
            break;
        }
    }

    let line_upper = line.to_uppercase();
    let (before, after) = match code.as_ref() {
        Some(c) => {
            let cu = c.to_uppercase();
            match line_upper.find(&cu) {
                Some(i) => (line[..i].to_string(), line[i + c.len()..].to_string()),
                None => (line.to_string(), String::new()),
            }
        }
        None => (line.to_string(), String::new()),
    };

    let before_clean = before.to_uppercase().replace('-', "");
    let mut gun: Option<String> = None;
    let mut best_len = 0usize;
    for k in known_gun_names(app) {
        let ku = k.to_uppercase().replace('-', "");
        if !ku.is_empty() && before_clean.contains(&ku) && ku.chars().count() > best_len {
            gun = Some(k.clone());
            best_len = ku.chars().count();
        }
    }
    let g = gun?;

    let mut note = after.trim().to_string();
    if note.is_empty() {
        let mut b = before.trim().to_string();
        b = b
            .replacen(&g.to_uppercase(), "", 1)
            .replace(&g.clone(), "")
            .trim()
            .to_string();
        note = b;
    }
    Some((g, note))
}

/// 从代码中识别枪名 + 配置名
pub fn recognize_weapon(app: &tauri::AppHandle, code: &str) -> Option<(String, String)> {
    // 0. 前缀映射识别（学习式，最优先，含配置名）
    if let Some(result) = recognize_by_prefix(app, code) {
        return Some(result);
    }
    // 1. 行内识别（与批量导入一致：已知枪名最长匹配 + 配置名提取）
    if let Some(result) = recognize_from_line(app, code) {
        return Some(result);
    }
    let upper = code.to_uppercase();

    // 2. 尝试提取 DELTA- 前缀格式的枪名（如 "DELTA-M4A1-001"）
    if let Some(start) = upper.find("DELTA-") {
        let rest = &upper[start + 6..];
        if let Some(end) = rest.find('-') {
            let candidate = &rest[..end];
            if !candidate.is_empty() && candidate.len() <= 20 {
                return Some((candidate.to_string(), String::new()));
            }
        }
    }

    // 3. 尝试提取 "-XXX-001" 模式（改枪码常见格式）
    let parts: Vec<&str> = upper.split('-').collect();
    for (i, part) in parts.iter().enumerate() {
        if part.len() >= 2 && part.len() <= 10 && i > 0 && parts.get(i + 1).map_or(false, |n| n.chars().all(|c| c.is_ascii_digit())) {
            if !part.chars().all(|c| c.is_ascii_digit()) {
                return Some((part.to_string(), String::new()));
            }
        }
    }

    None
}

// ═══════════════ 命令 ═══════════════

/// 获取全部本地改枪码
#[tauri::command]
pub fn get_local_gun_codes(app: tauri::AppHandle) -> Vec<LocalGunCode> {
    let mut items = load_all(&app);
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    items
}

/// 导入（新增）一条改枪码
#[tauri::command]
pub fn add_local_gun_code(app: tauri::AppHandle, weapon_name: String, code: String, note: String) -> Result<LocalGunCode, String> {
    if code.trim().is_empty() {
        return Err("改枪码不能为空".to_string());
    }

    let mut weapon = weapon_name.trim().to_string();
    let mut note = note.trim().to_string();

    // 自动识别：枪名或备注为空时，用识别结果补齐（识别包含配置名）
    if weapon.is_empty() || note.is_empty() {
        if let Some((w, n)) = recognize_weapon(&app, &code) {
            if weapon.is_empty() {
                weapon = w;
            }
            if note.is_empty() {
                note = n;
            }
        }
    }
    if weapon.is_empty() {
        weapon = "未知枪械".to_string();
    }

    // 学习前缀映射：记住 (代码前缀 → 枪名, 配置名)
    learn_prefix(&app, &code, &weapon, &note);

    let item = LocalGunCode {
        id: format!("{:x}", uuid::Uuid::new_v4().simple()),
        weapon_name: weapon,
        code: code.trim().to_string(),
        note,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    };

    let mut items = load_all(&app);
    items.push(item.clone());
    save_all(&app, &items);
    Ok(item)
}

/// 删除一条改枪码
#[tauri::command]
pub fn delete_local_gun_code(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let mut items = load_all(&app);
    let before = items.len();
    items.retain(|i| i.id != id);
    if items.len() == before {
        return Err("未找到该改枪码".to_string());
    }
    save_all(&app, &items);
    Ok(())
}

/// 更新一条改枪码
#[tauri::command]
pub fn update_local_gun_code(app: tauri::AppHandle, id: String, weapon_name: String, code: String, note: String) -> Result<(), String> {
    let mut items = load_all(&app);
    if let Some(item) = items.iter_mut().find(|i| i.id == id) {
        if !code.trim().is_empty() {
            item.code = code.trim().to_string();
        }
        if !weapon_name.trim().is_empty() {
            item.weapon_name = weapon_name.trim().to_string();
        }
        item.note = note.trim().to_string();
        save_all(&app, &items);
        Ok(())
    } else {
        Err("未找到该改枪码".to_string())
    }
}

/// 识别改枪码中的枪名 + 配置名（供前端粘贴时实时提示）
#[tauri::command]
pub fn recognize_gun_name(app: tauri::AppHandle, code: String) -> ImportResult {
    let recognized = recognize_weapon(&app, &code);
    ImportResult {
        weapon_name: recognized.clone().map(|r| r.0).unwrap_or_else(|| "未知枪械".to_string()),
        note: recognized.clone().map(|r| r.1).unwrap_or_default(),
        recognized: recognized.is_some(),
    }
}

/// 批量导入：从文档文本中解析全部改枪码（云更新场景）
#[tauri::command]
pub fn import_gun_codes_batch(app: tauri::AppHandle, text: String) -> Result<ImportBatchResult, String> {
    let mut result = ImportBatchResult::default();
    let mut existing = load_all(&app);
    let mut existing_codes: std::collections::HashSet<String> = existing.iter().map(|i| i.code.trim().to_uppercase()).collect();
    let mut added_any = false;

    // 收集全部已知枪名（内置 + 学习），用于行内识别
    let known = known_gun_names(&app);

    let mut last_gun: Option<String> = None;
    let mut last_desc: Option<String> = None;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            last_desc = None;
            continue;
        }

        // 1) "枪名...烽火地带-代码" 格式（如 MK47突击步枪-烽火地带-6HN54R8...）
        //    用 rfind("烽火地带") 定位，避免行首含 '-'（如 AS-VAL）时切错位置
        if let Some(fb_pos) = line.find("烽火地带") {
            let after = &line[fb_pos + "烽火地带".len()..];
            // 代码 = "烽火地带" 之后第一个 20+ 位字母数字串
            let mut code: Option<String> = None;
            for tok in after.split(|c: char| !c.is_ascii_alphanumeric()) {
                if tok.len() >= 20 && tok.len() <= 32 {
                    code = Some(tok.to_string());
                    break;
                }
            }
            if let Some(code) = code {
                let before_fb = &line[..fb_pos];

                // 去掉后缀词（突击步枪/战斗步枪等），剩 "AS Val"、"AR57" 这类枪名
                let mut clean = before_fb.to_string();
                for suffix in ["突击步枪", "战斗步枪", "狙击步枪", "冲锋枪", "步枪"] {
                    clean = clean.replace(suffix, "");
                }

                // 枪名 = known 集合最长匹配（不区分大小写，避免 AS Val 被拆、MK47 被 MK4 误匹配）
                let mut gun: Option<String> = None;
                let mut best_len = 0usize;
                // 去掉连字符再匹配（AR-57 / SCAR-H 等带 - 的枪名）
                let clean_upper = clean.to_uppercase().replace('-', "");
                for k in &known {
                    let ku = k.to_uppercase().replace('-', "");
                    if !ku.is_empty() && clean_upper.contains(&ku) && ku.chars().count() > best_len {
                        gun = Some(k.clone());
                        best_len = ku.chars().count();
                    }
                }
                if gun.is_none() {
                    gun = last_gun.clone();
                }

                // 备注 = 清掉枪名后的行内描述（取最后两个 token，如 "T1 50W 满改" → "50W 满改"）
                let mut note = String::new();
                if let Some(g) = &gun {
                    let after_gun = clean.replace(g, "");
                    let toks: Vec<&str> = after_gun
                        .split(|c: char| c == '\t' || c == ' ' || c == '　')
                        .filter(|s| !s.is_empty())
                        .collect();
                    if toks.len() >= 2 {
                        note = format!("{} {}", toks[toks.len() - 2], toks[toks.len() - 1]);
                    } else if toks.len() == 1 {
                        note = toks[0].to_string();
                    }
                }
                if note.is_empty() {
                    note = last_desc.clone().unwrap_or_default();
                }

                let weapon = gun.clone().unwrap_or_else(|| "未知枪械".to_string());
                let code_clean = code.to_string();
                let code_key = code_clean.to_uppercase();
                if !existing_codes.contains(&code_key) {
                    let item = LocalGunCode {
                        id: format!("{:x}", uuid::Uuid::new_v4().simple()),
                        weapon_name: weapon.clone(),
                        code: code_clean.clone(),
                        note: note.clone(),
                        created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
                    };
                    learn_prefix(&app, &code_clean, &weapon, &note);
                    existing.push(item);
                    existing_codes.insert(code_key);
                    result.imported += 1;
                    added_any = true;
                } else {
                    // 已存在：旧备注为空则补上
                    let mut updated = false;
                    if let Some(item) = existing.iter_mut().find(|i| i.code.trim().to_uppercase() == code_key) {
                        if item.note.is_empty() && !note.is_empty() {
                            item.note = note.clone();
                            updated = true;
                        }
                        if item.weapon_name == "未知枪械" && weapon != "未知枪械" {
                            item.weapon_name = weapon.clone();
                            updated = true;
                        }
                    }
                    if updated {
                        result.updated += 1;
                        added_any = true;
                    } else {
                        result.skipped += 1;
                    }
                }
                if gun.is_some() {
                    last_gun = gun;
                }
                last_desc = None;
                continue;
            }
        }

        // 2) 通用格式：行内找 20+ 位字母数字代码
        let clean_all: String = line.chars().filter(|c| c.is_ascii_alphanumeric() || c.is_whitespace()).collect();
        let mut code: Option<String> = None;
        for tok in clean_all.split_whitespace() {
            if tok.chars().all(|c| c.is_ascii_alphanumeric()) && tok.len() >= 20 && tok.len() <= 32 {
                code = Some(tok.to_string());
                break;
            }
        }
        if let Some(code) = code {
            // 该行去掉代码后的文本
            let line_upper = line.to_uppercase();
            let code_upper = code.to_uppercase();
            let before = line_upper.find(&code_upper).map(|i| line[..i].to_string()).unwrap_or_default();

            // 枪名：行内已知枪名优先（选最长匹配，避免 MK47 被 MK4 误匹配；去掉连字符匹配 AR-57/SCAR-H），否则继承
            let before_clean = before.to_uppercase().replace('-', "");
            let mut gun: Option<String> = None;
            let mut best_len = 0usize;
            for k in &known {
                let ku = k.to_uppercase().replace('-', "");
                if !ku.is_empty() && before_clean.contains(&ku) && ku.chars().count() > best_len {
                    gun = Some(k.clone());
                    best_len = ku.chars().count();
                }
            }
            if gun.is_none() {
                gun = last_gun.clone();
            }

            // 配置名：优先取「代码后」的文本（如 "M14 代码 红点" → 红点），
            // 其次代码前去掉枪名的文本，其次上一行描述
            let after = line_upper
                .find(&code_upper)
                .map(|i| line[i + code_upper.len()..].to_string())
                .unwrap_or_default();
            let mut note = after.trim().to_string();
            if note.is_empty() {
                note = before.trim().to_string();
                if let Some(g) = &gun {
                    note = note.replacen(&g.to_uppercase(), "", 1).replace(&g.clone(), "").trim().to_string();
                }
            }
            if note.is_empty() {
                note = last_desc.clone().unwrap_or_default();
            }

            let code_clean = code.to_string();
            let code_key = code_clean.to_uppercase();
            let weapon = gun.as_ref().cloned().unwrap_or_else(|| "未知枪械".to_string());
            if !existing_codes.contains(&code_key) {
                let item = LocalGunCode {
                    id: format!("{:x}", uuid::Uuid::new_v4().simple()),
                    weapon_name: weapon.clone(),
                    code: code_clean.clone(),
                    note: note.clone(),
                    created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
                };
                learn_prefix(&app, &code_clean, &weapon, &note);
                existing.push(item);
                existing_codes.insert(code_key);
                result.imported += 1;
                added_any = true;
            } else {
                // 已存在：如果旧记录备注为空而本次有备注，补上（避免重复导入永远补不上备注）
                let mut updated = false;
                if let Some(item) = existing.iter_mut().find(|i| i.code.trim().to_uppercase() == code_key) {
                    if item.note.is_empty() && !note.is_empty() {
                        item.note = note.clone();
                        updated = true;
                    }
                    if item.weapon_name == "未知枪械" && weapon != "未知枪械" {
                        item.weapon_name = weapon.clone();
                        updated = true;
                    }
                }
                if updated {
                    result.updated += 1;
                    added_any = true;
                } else {
                    result.skipped += 1;
                }
            }
            if gun.is_some() {
                last_gun = gun;
            }
            last_desc = None;
        } else {
            // 无代码的行：记录为描述（供下一行烽火地带/无配置行使用），过滤明显广告
            let l = line.to_string();
            if l.chars().count() <= 40 {
                last_desc = Some(l);
            } else {
                last_desc = None;
            }
        }
    }

    if added_any {
        save_all(&app, &existing);
    }
    Ok(result)
}

/// 批量导入结果
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct ImportBatchResult {
    pub imported: usize,
    pub updated: usize,
    pub skipped: usize,
}

// ═══════════════ 导出 / 导入备份 ═══════════════

/// 导出全部改枪码为 JSON 文件
#[tauri::command]
pub fn export_gun_codes(app: tauri::AppHandle, path: String) -> Result<usize, String> {
    let items = load_all(&app);
    let json = serde_json::to_string_pretty(&items).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("写入失败: {e}"))?;
    Ok(items.len())
}

/// 从 JSON 文件导入改枪码（按代码去重，合并到现有数据）
#[tauri::command]
pub fn import_gun_codes(app: tauri::AppHandle, path: String) -> Result<usize, String> {
    let content = std::fs::read_to_string(&path).map_err(|e| format!("读取失败: {e}"))?;
    let incoming: Vec<LocalGunCode> = serde_json::from_str(&content).map_err(|e| format!("JSON 解析失败: {e}"))?;

    let mut items = load_all(&app);
    let mut existing_codes: std::collections::HashSet<String> =
        items.iter().map(|i| i.code.trim().to_uppercase()).collect();

    let mut added = 0usize;
    let mut changed = false;
    for item in incoming {
        let key = item.code.trim().to_uppercase();
        if key.is_empty() {
            continue;
        }
        if !existing_codes.contains(&key) {
            // 学习前缀：导入的同时记住枪名+配置
            learn_prefix(&app, &item.code, &item.weapon_name, &item.note);
            items.push(item);
            existing_codes.insert(key);
            added += 1;
            changed = true;
        } else {
            // 已存在：旧备注为空则补上，枪名未知则补枪名
            if let Some(existing_item) = items.iter_mut().find(|i| i.code.trim().to_uppercase() == key) {
                let mut upd = false;
                if existing_item.note.is_empty() && !item.note.is_empty() {
                    existing_item.note = item.note.clone();
                    upd = true;
                }
                if existing_item.weapon_name == "未知枪械" && item.weapon_name != "未知枪械" {
                    existing_item.weapon_name = item.weapon_name.clone();
                    upd = true;
                }
                if upd {
                    changed = true;
                }
            }
        }
    }
    if changed {
        save_all(&app, &items);
    }
    Ok(added)
}
