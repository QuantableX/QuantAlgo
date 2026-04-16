use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine as _;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;
use chrono::{NaiveDate, NaiveDateTime, TimeZone, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Data Structures
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Strategy {
    pub id: String,
    pub name: String,
    pub description: String,
    pub file_path: String,
    pub params_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Trade {
    pub id: String,
    pub strategy_id: String,
    pub exchange: String,
    pub pair: String,
    pub side: String,
    pub entry_price: f64,
    pub exit_price: Option<f64>,
    pub quantity: f64,
    pub entry_time: String,
    pub exit_time: Option<String>,
    pub pnl: Option<f64>,
    pub pnl_pct: Option<f64>,
    pub fee: f64,
    pub is_backtest: bool,
    pub backtest_id: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BacktestConfig {
    pub strategy_id: String,
    pub exchange: String,
    pub pair: String,
    pub timeframe: String,
    pub start_date: String,
    pub end_date: String,
    pub initial_capital: f64,
    pub commission: f64,
    #[serde(default)]
    pub strategy_params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BacktestStats {
    pub total_return: f64,
    pub total_return_pct: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub max_drawdown_pct: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub total_trades: i64,
    pub avg_trade_duration_secs: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EquityPoint {
    pub time: String,
    pub equity: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BacktestResult {
    pub id: String,
    pub name: String,
    pub strategy_id: String,
    pub config: BacktestConfig,
    pub stats: BacktestStats,
    pub equity_curve: Vec<EquityPoint>,
    pub trades: Vec<Trade>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BacktestSummary {
    pub id: String,
    pub name: String,
    pub strategy_id: String,
    pub config_json: String,
    pub stats_json: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Exchange {
    pub id: String,
    pub name: String,
    pub exchange_type: String,
    pub provider: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExchangeConfig {
    pub name: String,
    pub exchange_type: String,
    pub provider: String,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub passphrase: Option<String>,
    pub wallet_address: Option<String>,
    pub private_key: Option<String>,
    pub rpc_endpoint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConnectionResult {
    pub success: bool,
    pub message: String,
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Balance {
    pub asset: String,
    pub total: f64,
    pub available: f64,
    pub in_positions: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BotStatus {
    pub status: String,
    pub strategy_id: Option<String>,
    pub exchange_id: Option<String>,
    pub pair: Option<String>,
    pub started_at: Option<String>,
    pub config_json: Option<String>,
    pub trading_mode: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub theme: String,
    pub font_size: u32,
    pub default_pair: String,
    pub default_timeframe: String,
    pub python_path: String,
    pub strategy_dir: String,
    pub backtest_dir: String,
    pub risk_per_trade: f64,
    pub max_concurrent_positions: u32,
    pub slippage_tolerance: f64,
    #[serde(default = "default_paper_fee_pct")]
    pub paper_fee_pct: f64,
    pub notify_on_trade: bool,
    pub notify_on_error: bool,
    pub notify_on_daily_summary: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TradeFilters {
    pub strategy_id: Option<String>,
    pub exchange: Option<String>,
    pub pair: Option<String>,
    pub side: Option<String>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub min_pnl: Option<f64>,
    pub is_backtest: Option<bool>,
    pub backtest_id: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TradeStats {
    pub total_trades: f64,
    pub win_rate: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub profit_factor: f64,
    pub expectancy: f64,
    pub best_trade: f64,
    pub worst_trade: f64,
    pub total_pnl: f64,
    pub total_pnl_pct: f64,
    pub avg_duration_secs: f64,
}

// ---------------------------------------------------------------------------
// App State
// ---------------------------------------------------------------------------

pub struct BotProcess {
    pub child: Arc<Mutex<Child>>,
    pub stdin: Arc<Mutex<ChildStdin>>,
    pub stop_flag: Arc<AtomicBool>,
}

pub struct AppState {
    pub db: Mutex<Connection>,
    pub bot_process: Mutex<Option<BotProcess>>,
    pub settings: Mutex<AppSettings>,
    pub bot_logs: Mutex<Vec<LogEntry>>,
}

#[derive(Debug, Clone)]
struct LivePosition {
    id: String,
    pair: String,
    side: String,
    entry_price: f64,
    quantity: f64,
    entry_fee: f64,
    reserved_margin: f64,
    entry_time: String,
}

#[derive(Debug)]
struct LiveBotRuntime {
    strategy_id: String,
    exchange_id: String,
    pair: String,
    balance: f64,
    last_price: f64,
    open_positions: Vec<LivePosition>,
    fee_rate: f64,
    slippage_pct: f64,
    risk_per_trade: f64,
    max_positions: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct StrategyExport {
    metadata: Strategy,
    code: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct BacktestExportRow {
    id: String,
    name: String,
    strategy_id: String,
    config_json: String,
    stats_json: String,
    equity_curve_json: String,
    created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExchangeExportRow {
    exchange: Exchange,
    config_encrypted: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct EquitySnapshotRow {
    timestamp: String,
    equity: f64,
    source: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct DataExport {
    exported_at: String,
    settings: AppSettings,
    strategies: Vec<StrategyExport>,
    backtests: Vec<BacktestExportRow>,
    exchanges: Vec<ExchangeExportRow>,
    trades: Vec<Trade>,
    equity_snapshots: Vec<EquitySnapshotRow>,
    bot_state: BotStatus,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const DEFAULT_RISK_PER_TRADE_PCT: f64 = 1.0;
const DEFAULT_MAX_CONCURRENT_POSITIONS: u32 = 3;
const DEFAULT_SLIPPAGE_TOLERANCE_PCT: f64 = 0.1;
const DEFAULT_PAPER_FEE_PCT: f64 = 0.1;

const MAX_RISK_PER_TRADE_PCT: f64 = 10.0;
const WARN_RISK_PER_TRADE_PCT: f64 = 5.0;
const MAX_CONCURRENT_POSITIONS: u64 = 20;
const WARN_CONCURRENT_POSITIONS: u64 = 10;
const MAX_SLIPPAGE_TOLERANCE_PCT: f64 = 5.0;
const WARN_SLIPPAGE_TOLERANCE_PCT: f64 = 1.0;
const MAX_PAPER_FEE_PCT: f64 = 5.0;
const WARN_PAPER_FEE_PCT: f64 = 1.0;
const PAIR_CACHE_TTL_SECS: i64 = 15 * 60;

fn default_paper_fee_pct() -> f64 {
    DEFAULT_PAPER_FEE_PCT
}

#[derive(Clone)]
struct CachedPairs {
    fetched_at: chrono::DateTime<Utc>,
    pairs: Vec<String>,
}

static PAIR_CACHE: OnceLock<Mutex<HashMap<String, CachedPairs>>> = OnceLock::new();

fn pair_cache() -> &'static Mutex<HashMap<String, CachedPairs>> {
    PAIR_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn get_data_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("quantalgo");
    std::fs::create_dir_all(&dir).ok();
    dir
}

pub fn get_default_settings() -> AppSettings {
    let data_dir = get_data_dir();
    AppSettings {
        theme: "dark".into(),
        font_size: 14,
        default_pair: "BTC/USDT".into(),
        default_timeframe: "1h".into(),
        python_path: if cfg!(windows) {
            "py".into()
        } else {
            "python3".into()
        },
        strategy_dir: data_dir.join("strategies").to_string_lossy().into_owned(),
        backtest_dir: data_dir.join("backtests").to_string_lossy().into_owned(),
        risk_per_trade: DEFAULT_RISK_PER_TRADE_PCT,
        max_concurrent_positions: DEFAULT_MAX_CONCURRENT_POSITIONS,
        slippage_tolerance: DEFAULT_SLIPPAGE_TOLERANCE_PCT,
        paper_fee_pct: DEFAULT_PAPER_FEE_PCT,
        notify_on_trade: true,
        notify_on_error: true,
        notify_on_daily_summary: false,
    }
}

const LEGACY_APP_ENCRYPTION_KEY: &[u8; 32] = b"QuantAlgo_AES256_Key_2024!@#$%^&";

fn derive_encryption_key() -> [u8; 32] {
    let user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "local-user".to_string());
    let material = format!("quantalgo:v2:{}:{}", user, get_data_dir().to_string_lossy());
    let digest = Sha256::digest(material.as_bytes());
    let mut key = [0u8; 32];
    key.copy_from_slice(&digest);
    key
}

pub fn encrypt_string(plaintext: &str) -> Result<String, String> {
    let key = derive_encryption_key();
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("Cipher init: {e}"))?;
    let nonce_bytes: [u8; 12] = {
        use aes_gcm::aead::rand_core::RngCore;
        let mut buf = [0u8; 12];
        OsRng.fill_bytes(&mut buf);
        buf
    };
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("Encrypt: {e}"))?;
    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);
    Ok(hex::encode(combined))
}

fn decrypt_with_key(data: &[u8], key: &[u8; 32]) -> Result<String, String> {
    if data.len() < 13 {
        return Err("Ciphertext too short".into());
    }
    let (nonce_bytes, ciphertext) = data.split_at(12);
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| format!("Cipher init: {e}"))?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decrypt: {e}"))?;
    String::from_utf8(plaintext).map_err(|e| format!("UTF-8: {e}"))
}

pub fn decrypt_string(hex_str: &str) -> Result<String, String> {
    let data = hex::decode(hex_str).map_err(|e| format!("Hex decode: {e}"))?;
    let key = derive_encryption_key();
    decrypt_with_key(&data, &key).or_else(|_| decrypt_with_key(&data, LEGACY_APP_ENCRYPTION_KEY))
}

fn hmac_sha256_hex(key: &[u8], message: &[u8]) -> String {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).expect("HMAC key length");
    mac.update(message);
    hex::encode(mac.finalize().into_bytes())
}

fn hmac_sha256_base64(key: &[u8], message: &[u8]) -> String {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).expect("HMAC key length");
    mac.update(message);
    base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}

/// Fetch the exchange's server time in milliseconds to avoid clock drift rejections.
/// Falls back to local time if the request fails.
fn get_server_time_ms(client: &reqwest::blocking::Client, provider: &str) -> i64 {
    let fallback = chrono::Utc::now().timestamp_millis();
    let result: Option<i64> = match provider {
        "binance" => client
            .get("https://api.binance.com/api/v3/time")
            .send()
            .ok()
            .and_then(|r| r.json::<Value>().ok())
            .and_then(|j| j.get("serverTime").and_then(|v| v.as_i64())),
        "bybit" => client
            .get("https://api.bybit.com/v5/market/time")
            .send()
            .ok()
            .and_then(|r| r.json::<Value>().ok())
            .and_then(|j| {
                j.pointer("/result/timeSecond")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok())
                    .map(|s| (s * 1000.0) as i64)
                    .or_else(|| j.get("time").and_then(|v| v.as_i64()))
            }),
        "kucoin" => client
            .get("https://api.kucoin.com/api/v1/timestamp")
            .send()
            .ok()
            .and_then(|r| r.json::<Value>().ok())
            .and_then(|j| j.get("data").and_then(|v| v.as_i64())),
        _ => None,
    };
    result.unwrap_or(fallback)
}

fn load_settings_from_disk() -> AppSettings {
    let path = get_data_dir().join("config.json");
    let mut settings = if let Ok(data) = std::fs::read_to_string(&path) {
        serde_json::from_str(&data).unwrap_or_else(|_| get_default_settings())
    } else {
        get_default_settings()
    };

    if normalize_settings_units(&mut settings) {
        let _ = save_settings_to_disk(&settings);
    }
    settings
}

fn save_settings_to_disk(settings: &AppSettings) -> Result<(), String> {
    let path = get_data_dir().join("config.json");
    let json = serde_json::to_string_pretty(settings).map_err(|e| format!("Serialize: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Write config: {e}"))
}

fn normalize_settings_units(settings: &mut AppSettings) -> bool {
    let mut changed = false;

    if settings.risk_per_trade > 0.0 && settings.risk_per_trade < 0.1 {
        settings.risk_per_trade *= 100.0;
        changed = true;
    }
    if settings.slippage_tolerance > 0.0 && settings.slippage_tolerance < 0.01 {
        settings.slippage_tolerance *= 100.0;
        changed = true;
    }
    if settings.paper_fee_pct < 0.0 {
        settings.paper_fee_pct = DEFAULT_PAPER_FEE_PCT;
        changed = true;
    }
    if settings.max_concurrent_positions == 0 {
        settings.max_concurrent_positions = DEFAULT_MAX_CONCURRENT_POSITIONS;
        changed = true;
    }

    changed
}

fn validate_app_settings(settings: &AppSettings) -> Result<(), String> {
    if !(settings.risk_per_trade > 0.0 && settings.risk_per_trade <= MAX_RISK_PER_TRADE_PCT) {
        return Err(format!(
            "Risk per trade must be greater than 0 and no more than {MAX_RISK_PER_TRADE_PCT:.0}%."
        ));
    }
    if !(1..=MAX_CONCURRENT_POSITIONS).contains(&(settings.max_concurrent_positions as u64)) {
        return Err(format!(
            "Max concurrent positions must be between 1 and {MAX_CONCURRENT_POSITIONS}."
        ));
    }
    if !(settings.slippage_tolerance >= 0.0
        && settings.slippage_tolerance <= MAX_SLIPPAGE_TOLERANCE_PCT)
    {
        return Err(format!(
            "Slippage tolerance must be between 0 and {MAX_SLIPPAGE_TOLERANCE_PCT:.0}%."
        ));
    }
    if !(settings.paper_fee_pct >= 0.0 && settings.paper_fee_pct <= MAX_PAPER_FEE_PCT) {
        return Err(format!(
            "Paper fee must be between 0 and {MAX_PAPER_FEE_PCT:.0}%."
        ));
    }
    Ok(())
}

fn get_workspace_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|path| path.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn get_python_sdk_dir() -> PathBuf {
    get_workspace_dir().join("python")
}

fn resolve_python_path(settings: &AppSettings) -> String {
    let configured = settings.python_path.trim();
    if configured.is_empty() {
        if cfg!(windows) {
            "py".into()
        } else {
            "python3".into()
        }
    } else {
        configured.to_string()
    }
}

fn build_python_command(python_path: &str) -> Command {
    let mut command = Command::new(python_path);
    let python_path_buf = PathBuf::from(python_path);
    let file_name = python_path_buf
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(python_path);

    if cfg!(windows)
        && (file_name.eq_ignore_ascii_case("py") || file_name.eq_ignore_ascii_case("py.exe"))
    {
        command.arg("-3");
    }

    command.arg("-u");

    let sdk_dir = get_python_sdk_dir();
    let mut python_path_env = OsString::from(sdk_dir);
    if let Some(existing) = std::env::var_os("PYTHONPATH") {
        python_path_env.push(if cfg!(windows) { ";" } else { ":" });
        python_path_env.push(existing);
    }

    command.env("PYTHONPATH", python_path_env);
    command
}

fn parse_date_to_utc(input: &str, end_of_day: bool) -> Result<chrono::DateTime<Utc>, String> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(input) {
        return Ok(dt.with_timezone(&Utc));
    }

    if let Ok(ndt) = NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S") {
        return Ok(Utc.from_utc_datetime(&ndt));
    }

    let date = NaiveDate::parse_from_str(input, "%Y-%m-%d")
        .map_err(|e| format!("Invalid date '{input}': {e}"))?;
    let time = if end_of_day {
        date.and_hms_opt(23, 59, 59)
    } else {
        date.and_hms_opt(0, 0, 0)
    }
    .ok_or_else(|| format!("Invalid date '{input}'"))?;

    Ok(Utc.from_utc_datetime(&time))
}

fn timeframe_seconds(timeframe: &str) -> i64 {
    match timeframe {
        "1m" => 60,
        "5m" => 300,
        "15m" => 900,
        "1h" => 3600,
        "4h" => 14_400,
        "1d" => 86_400,
        "1w" => 604_800,
        _ => 3600,
    }
}

#[derive(Debug, Clone)]
struct PaperMarketCandle {
    time: String,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

// ---------------------------------------------------------------------------
// Real exchange candle fetching
// ---------------------------------------------------------------------------

fn pair_for_exchange(pair: &str, provider: &str) -> String {
    // "BTC/USDT" -> exchange-specific format
    match provider {
        "binance" => pair.replace('/', ""),   // BTCUSDT
        "bybit" => pair.replace('/', ""),     // BTCUSDT
        "okx" => pair.replace('/', "-"),      // BTC-USDT
        "coinbase" => pair.replace('/', "-"), // BTC-USDT
        "kucoin" => pair.replace('/', "-"),   // BTC-USDT
        "kraken" => pair.replace("BTC", "XBT").replace('/', ""), // XBTUSDT
        _ => pair.replace('/', ""),
    }
}

fn interval_for_exchange(timeframe: &str, provider: &str) -> String {
    match provider {
        "binance" => timeframe.to_string(), // 1m, 5m, 15m, 1h, 4h, 1d
        "bybit" => match timeframe {
            // 1, 5, 15, 60, 240, D
            "1m" => "1",
            "5m" => "5",
            "15m" => "15",
            "1h" => "60",
            "4h" => "240",
            "1d" => "D",
            "1w" => "W",
            _ => "60",
        }
        .to_string(),
        "okx" => match timeframe {
            // 1m, 5m, 15m, 1H, 4H, 1D
            "1h" => "1H".to_string(),
            "4h" => "4H".to_string(),
            "1d" => "1D".to_string(),
            "1w" => "1W".to_string(),
            other => other.to_string(),
        },
        "coinbase" => match timeframe {
            // seconds: 60, 300, 900, 3600, 21600, 86400
            "1m" => "60",
            "5m" => "300",
            "15m" => "900",
            "1h" => "3600",
            "4h" => "21600",
            "1d" => "86400",
            _ => "3600",
        }
        .to_string(),
        "kraken" => match timeframe {
            // 1, 5, 15, 30, 60, 240, 1440, 10080
            "1m" => "1",
            "5m" => "5",
            "15m" => "15",
            "1h" => "60",
            "4h" => "240",
            "1d" => "1440",
            "1w" => "10080",
            _ => "60",
        }
        .to_string(),
        "kucoin" => match timeframe {
            "1m" => "1min",
            "5m" => "5min",
            "15m" => "15min",
            "1h" => "1hour",
            "4h" => "4hour",
            "1d" => "1day",
            "1w" => "1week",
            _ => "1hour",
        }
        .to_string(),
        _ => timeframe.to_string(),
    }
}

fn value_as_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|raw| raw.parse::<f64>().ok()))
}

fn value_as_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|raw| raw.parse::<i64>().ok()))
}

fn candle_time_from_millis(ts_ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ts_ms)
        .unwrap_or_else(Utc::now)
        .to_rfc3339()
}

fn candle_time_from_seconds(ts_secs: i64) -> String {
    chrono::DateTime::from_timestamp(ts_secs, 0)
        .unwrap_or_else(Utc::now)
        .to_rfc3339()
}

fn candle_from_array(
    row: &[Value],
    ts_idx: usize,
    open_idx: usize,
    high_idx: usize,
    low_idx: usize,
    close_idx: usize,
    volume_idx: usize,
    timestamp_in_seconds: bool,
) -> Result<PaperMarketCandle, String> {
    let ts = row
        .get(ts_idx)
        .and_then(value_as_i64)
        .ok_or_else(|| "candle timestamp missing".to_string())?;
    let open = row
        .get(open_idx)
        .and_then(value_as_f64)
        .ok_or_else(|| "candle open missing".to_string())?;
    let high = row
        .get(high_idx)
        .and_then(value_as_f64)
        .ok_or_else(|| "candle high missing".to_string())?;
    let low = row
        .get(low_idx)
        .and_then(value_as_f64)
        .ok_or_else(|| "candle low missing".to_string())?;
    let close = row
        .get(close_idx)
        .and_then(value_as_f64)
        .ok_or_else(|| "candle close missing".to_string())?;
    let volume = row.get(volume_idx).and_then(value_as_f64).unwrap_or(0.0);

    Ok(PaperMarketCandle {
        time: if timestamp_in_seconds {
            candle_time_from_seconds(ts)
        } else {
            candle_time_from_millis(ts)
        },
        open,
        high,
        low,
        close,
        volume,
    })
}

fn fetch_latest_market_candle(
    provider: &str,
    pair: &str,
    timeframe: &str,
) -> Result<PaperMarketCandle, String> {
    let provider = provider.to_lowercase();
    let symbol = pair_for_exchange(pair, &provider);
    let interval = interval_for_exchange(timeframe, &provider);
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;

    match provider.as_str() {
        "binance" => {
            let resp = client
                .get("https://api.binance.com/api/v3/klines")
                .query(&[
                    ("symbol", symbol.as_str()),
                    ("interval", interval.as_str()),
                    ("limit", "2"),
                ])
                .send()
                .map_err(|e| format!("Binance market data request failed: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("Binance market data HTTP {}", resp.status()));
            }
            let body: Vec<Vec<Value>> = resp
                .json()
                .map_err(|e| format!("Binance market data parse: {e}"))?;
            let row = body
                .last()
                .ok_or_else(|| "Binance returned no candles".to_string())?;
            candle_from_array(row, 0, 1, 2, 3, 4, 5, false)
        }
        "bybit" => {
            let resp = client
                .get("https://api.bybit.com/v5/market/kline")
                .query(&[
                    ("category", "spot"),
                    ("symbol", symbol.as_str()),
                    ("interval", interval.as_str()),
                    ("limit", "2"),
                ])
                .send()
                .map_err(|e| format!("Bybit market data request failed: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("Bybit market data HTTP {}", resp.status()));
            }
            let body: Value = resp
                .json()
                .map_err(|e| format!("Bybit market data parse: {e}"))?;
            if body
                .get("retCode")
                .and_then(|value| value.as_i64())
                .unwrap_or(0)
                != 0
            {
                let msg = body
                    .get("retMsg")
                    .and_then(|value| value.as_str())
                    .unwrap_or("unknown error");
                return Err(format!("Bybit market data error: {msg}"));
            }
            let list = body
                .pointer("/result/list")
                .and_then(|value| value.as_array())
                .ok_or_else(|| "Bybit returned no candles".to_string())?;
            let row = list
                .first()
                .and_then(|value| value.as_array())
                .ok_or_else(|| "Bybit returned malformed candle".to_string())?;
            candle_from_array(row, 0, 1, 2, 3, 4, 5, false)
        }
        "okx" => {
            let resp = client
                .get("https://www.okx.com/api/v5/market/candles")
                .query(&[
                    ("instId", symbol.as_str()),
                    ("bar", interval.as_str()),
                    ("limit", "2"),
                ])
                .send()
                .map_err(|e| format!("OKX market data request failed: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("OKX market data HTTP {}", resp.status()));
            }
            let body: Value = resp
                .json()
                .map_err(|e| format!("OKX market data parse: {e}"))?;
            let data = body
                .get("data")
                .and_then(|value| value.as_array())
                .ok_or_else(|| "OKX returned no candles".to_string())?;
            let row = data
                .first()
                .and_then(|value| value.as_array())
                .ok_or_else(|| "OKX returned malformed candle".to_string())?;
            candle_from_array(row, 0, 1, 2, 3, 4, 5, false)
        }
        "coinbase" => {
            let url = format!("https://api.exchange.coinbase.com/products/{symbol}/candles");
            let resp = client
                .get(&url)
                .query(&[("granularity", interval.as_str())])
                .send()
                .map_err(|e| format!("Coinbase market data request failed: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("Coinbase market data HTTP {}", resp.status()));
            }
            let body: Vec<Vec<Value>> = resp
                .json()
                .map_err(|e| format!("Coinbase market data parse: {e}"))?;
            let row = body
                .iter()
                .max_by_key(|row| row.first().and_then(value_as_i64).unwrap_or(0))
                .ok_or_else(|| "Coinbase returned no candles".to_string())?;
            // Coinbase candle layout: [time, low, high, open, close, volume].
            candle_from_array(row, 0, 3, 2, 1, 4, 5, true)
        }
        "kraken" => {
            let resp = client
                .get("https://api.kraken.com/0/public/OHLC")
                .query(&[("pair", symbol.as_str()), ("interval", interval.as_str())])
                .send()
                .map_err(|e| format!("Kraken market data request failed: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("Kraken market data HTTP {}", resp.status()));
            }
            let body: Value = resp
                .json()
                .map_err(|e| format!("Kraken market data parse: {e}"))?;
            if let Some(err) = body.get("error").and_then(|value| value.as_array()) {
                if !err.is_empty() {
                    return Err(format!("Kraken market data error: {:?}", err));
                }
            }
            let result = body
                .get("result")
                .and_then(|value| value.as_object())
                .ok_or_else(|| "Kraken returned no result".to_string())?;
            let pair_data = result
                .iter()
                .find(|(key, _)| key.as_str() != "last")
                .and_then(|(_, value)| value.as_array())
                .ok_or_else(|| "Kraken returned no candles".to_string())?;
            let row = pair_data
                .last()
                .and_then(|value| value.as_array())
                .ok_or_else(|| "Kraken returned malformed candle".to_string())?;
            // Kraken candle layout: [time, open, high, low, close, vwap, volume, count].
            candle_from_array(row, 0, 1, 2, 3, 4, 6, true)
        }
        "kucoin" => {
            let resp = client
                .get("https://api.kucoin.com/api/v1/market/candles")
                .query(&[("type", interval.as_str()), ("symbol", symbol.as_str())])
                .send()
                .map_err(|e| format!("KuCoin market data request failed: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("KuCoin market data HTTP {}", resp.status()));
            }
            let body: Value = resp
                .json()
                .map_err(|e| format!("KuCoin market data parse: {e}"))?;
            if body
                .get("code")
                .and_then(|value| value.as_str())
                .unwrap_or("200000")
                != "200000"
            {
                return Err(format!("KuCoin market data error: {}", body));
            }
            let data = body
                .get("data")
                .and_then(|value| value.as_array())
                .ok_or_else(|| "KuCoin returned no candles".to_string())?;
            let row = data
                .first()
                .and_then(|value| value.as_array())
                .ok_or_else(|| "KuCoin returned malformed candle".to_string())?;
            // KuCoin candle layout: [time, open, close, high, low, volume, turnover].
            candle_from_array(row, 0, 1, 3, 4, 2, 5, true)
        }
        other => Err(format!(
            "Public market data is not supported for {other}; choose a supported CEX provider."
        )),
    }
}

fn sort_and_trim_recent_candles(
    mut candles: Vec<PaperMarketCandle>,
    limit: usize,
) -> Vec<PaperMarketCandle> {
    candles.sort_by(|a, b| a.time.cmp(&b.time));
    candles.dedup_by(|a, b| a.time == b.time);
    if candles.len() > limit {
        candles.split_off(candles.len() - limit)
    } else {
        candles
    }
}

fn fetch_recent_market_candles(
    provider: &str,
    pair: &str,
    timeframe: &str,
    limit: usize,
) -> Result<Vec<PaperMarketCandle>, String> {
    let provider = provider.to_lowercase();
    let symbol = pair_for_exchange(pair, &provider);
    let interval = interval_for_exchange(timeframe, &provider);
    let limit = limit.clamp(2, 500);
    let limit_s = limit.to_string();
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;

    let candles = match provider.as_str() {
        "binance" => {
            let resp = client
                .get("https://api.binance.com/api/v3/klines")
                .query(&[
                    ("symbol", symbol.as_str()),
                    ("interval", interval.as_str()),
                    ("limit", limit_s.as_str()),
                ])
                .send()
                .map_err(|e| format!("Binance warm-up request failed: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("Binance warm-up HTTP {}", resp.status()));
            }
            let body: Vec<Vec<Value>> = resp
                .json()
                .map_err(|e| format!("Binance warm-up parse: {e}"))?;
            body.iter()
                .filter_map(|row| candle_from_array(row, 0, 1, 2, 3, 4, 5, false).ok())
                .collect::<Vec<_>>()
        }
        "bybit" => {
            let resp = client
                .get("https://api.bybit.com/v5/market/kline")
                .query(&[
                    ("category", "spot"),
                    ("symbol", symbol.as_str()),
                    ("interval", interval.as_str()),
                    ("limit", limit_s.as_str()),
                ])
                .send()
                .map_err(|e| format!("Bybit warm-up request failed: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("Bybit warm-up HTTP {}", resp.status()));
            }
            let body: Value = resp
                .json()
                .map_err(|e| format!("Bybit warm-up parse: {e}"))?;
            if body
                .get("retCode")
                .and_then(|value| value.as_i64())
                .unwrap_or(0)
                != 0
            {
                let msg = body
                    .get("retMsg")
                    .and_then(|value| value.as_str())
                    .unwrap_or("unknown error");
                return Err(format!("Bybit warm-up error: {msg}"));
            }
            body.pointer("/result/list")
                .and_then(|value| value.as_array())
                .map(|rows| {
                    rows.iter()
                        .filter_map(|value| value.as_array())
                        .filter_map(|row| candle_from_array(row, 0, 1, 2, 3, 4, 5, false).ok())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        }
        "okx" => {
            let resp = client
                .get("https://www.okx.com/api/v5/market/candles")
                .query(&[
                    ("instId", symbol.as_str()),
                    ("bar", interval.as_str()),
                    ("limit", limit_s.as_str()),
                ])
                .send()
                .map_err(|e| format!("OKX warm-up request failed: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("OKX warm-up HTTP {}", resp.status()));
            }
            let body: Value = resp.json().map_err(|e| format!("OKX warm-up parse: {e}"))?;
            body.get("data")
                .and_then(|value| value.as_array())
                .map(|rows| {
                    rows.iter()
                        .filter_map(|value| value.as_array())
                        .filter_map(|row| candle_from_array(row, 0, 1, 2, 3, 4, 5, false).ok())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        }
        "coinbase" => {
            let url = format!("https://api.exchange.coinbase.com/products/{symbol}/candles");
            let end = Utc::now();
            let start =
                end - chrono::Duration::seconds(timeframe_seconds(timeframe) * limit as i64);
            let start_s = start.to_rfc3339();
            let end_s = end.to_rfc3339();
            let resp = client
                .get(&url)
                .query(&[
                    ("granularity", interval.as_str()),
                    ("start", start_s.as_str()),
                    ("end", end_s.as_str()),
                ])
                .send()
                .map_err(|e| format!("Coinbase warm-up request failed: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("Coinbase warm-up HTTP {}", resp.status()));
            }
            let body: Vec<Vec<Value>> = resp
                .json()
                .map_err(|e| format!("Coinbase warm-up parse: {e}"))?;
            body.iter()
                .filter_map(|row| candle_from_array(row, 0, 3, 2, 1, 4, 5, true).ok())
                .collect::<Vec<_>>()
        }
        "kraken" => {
            let since = (Utc::now()
                - chrono::Duration::seconds(timeframe_seconds(timeframe) * limit as i64))
            .timestamp()
            .to_string();
            let resp = client
                .get("https://api.kraken.com/0/public/OHLC")
                .query(&[
                    ("pair", symbol.as_str()),
                    ("interval", interval.as_str()),
                    ("since", since.as_str()),
                ])
                .send()
                .map_err(|e| format!("Kraken warm-up request failed: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("Kraken warm-up HTTP {}", resp.status()));
            }
            let body: Value = resp
                .json()
                .map_err(|e| format!("Kraken warm-up parse: {e}"))?;
            if let Some(err) = body.get("error").and_then(|value| value.as_array()) {
                if !err.is_empty() {
                    return Err(format!("Kraken warm-up error: {:?}", err));
                }
            }
            body.get("result")
                .and_then(|value| value.as_object())
                .and_then(|result| {
                    result
                        .iter()
                        .find(|(key, _)| key.as_str() != "last")
                        .and_then(|(_, value)| value.as_array())
                })
                .map(|rows| {
                    rows.iter()
                        .filter_map(|value| value.as_array())
                        .filter_map(|row| candle_from_array(row, 0, 1, 2, 3, 4, 6, true).ok())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        }
        "kucoin" => {
            let end = Utc::now().timestamp().to_string();
            let start = (Utc::now()
                - chrono::Duration::seconds(timeframe_seconds(timeframe) * limit as i64))
            .timestamp()
            .to_string();
            let resp = client
                .get("https://api.kucoin.com/api/v1/market/candles")
                .query(&[
                    ("type", interval.as_str()),
                    ("symbol", symbol.as_str()),
                    ("startAt", start.as_str()),
                    ("endAt", end.as_str()),
                ])
                .send()
                .map_err(|e| format!("KuCoin warm-up request failed: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("KuCoin warm-up HTTP {}", resp.status()));
            }
            let body: Value = resp
                .json()
                .map_err(|e| format!("KuCoin warm-up parse: {e}"))?;
            if body
                .get("code")
                .and_then(|value| value.as_str())
                .unwrap_or("200000")
                != "200000"
            {
                return Err(format!("KuCoin warm-up error: {}", body));
            }
            body.get("data")
                .and_then(|value| value.as_array())
                .map(|rows| {
                    rows.iter()
                        .filter_map(|value| value.as_array())
                        .filter_map(|row| candle_from_array(row, 0, 1, 3, 4, 2, 5, true).ok())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        }
        other => {
            return Err(format!(
                "Public market data warm-up is not supported for {other}; choose a supported CEX provider."
            ));
        }
    };

    let candles = sort_and_trim_recent_candles(candles, limit);
    if candles.is_empty() {
        Err(format!(
            "No warm-up candles returned from {provider} for {pair} ({timeframe})."
        ))
    } else {
        Ok(candles)
    }
}

fn market_candle_json(candle: &PaperMarketCandle, pair: &str) -> Value {
    serde_json::json!({
        "time": candle.time.clone(),
        "open": candle.open,
        "high": candle.high,
        "low": candle.low,
        "close": candle.close,
        "volume": candle.volume,
        "pair": pair,
    })
}

fn fetch_historical_candles(config: &BacktestConfig) -> Result<Vec<Value>, String> {
    let provider = config.exchange.to_lowercase();
    let start = parse_date_to_utc(&config.start_date, false)?;
    let end = parse_date_to_utc(&config.end_date, true)?;
    let symbol = pair_for_exchange(&config.pair, &provider);
    let interval = interval_for_exchange(&config.timeframe, &provider);
    let interval_ms = timeframe_seconds(&config.timeframe) * 1000;

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;

    let mut all_candles: Vec<Value> = Vec::new();
    let mut cursor_ms = start.timestamp_millis();
    let end_ms = end.timestamp_millis();
    let max_per_page: i64 = match provider.as_str() {
        "binance" => 1000,
        "bybit" => 200,
        "okx" => 100,
        "coinbase" => 300,
        "kraken" => 720,
        _ => 1000,
    };

    loop {
        if cursor_ms >= end_ms {
            break;
        }

        let raw: Vec<Value> = match provider.as_str() {
            "binance" => {
                let resp = client
                    .get("https://api.binance.com/api/v3/klines")
                    .query(&[
                        ("symbol", symbol.as_str()),
                        ("interval", interval.as_str()),
                        ("limit", &max_per_page.to_string()),
                    ])
                    .query(&[
                        ("startTime", &cursor_ms.to_string()),
                        ("endTime", &end_ms.to_string()),
                    ])
                    .send()
                    .map_err(|e| format!("Binance request: {e}"))?;
                if !resp.status().is_success() {
                    return Err(format!("Binance API error: HTTP {}", resp.status()));
                }
                let body: Vec<Vec<Value>> =
                    resp.json().map_err(|e| format!("Binance parse: {e}"))?;
                body.into_iter()
                    .map(|k| {
                        let ts_ms = k[0].as_i64().unwrap_or(0);
                        let time = chrono::DateTime::from_timestamp_millis(ts_ms)
                            .unwrap_or(Utc::now())
                            .to_rfc3339();
                        serde_json::json!({
                            "time": time,
                            "open": k[1].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                            "high": k[2].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                            "low": k[3].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                            "close": k[4].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                            "volume": k[5].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                        })
                    })
                    .collect()
            }
            "bybit" => {
                let resp = client
                    .get("https://api.bybit.com/v5/market/kline")
                    .query(&[
                        ("category", "spot"),
                        ("symbol", &symbol),
                        ("interval", &interval),
                        ("limit", &max_per_page.to_string()),
                    ])
                    .query(&[
                        ("start", &cursor_ms.to_string()),
                        ("end", &end_ms.to_string()),
                    ])
                    .send()
                    .map_err(|e| format!("Bybit request: {e}"))?;
                if !resp.status().is_success() {
                    return Err(format!("Bybit API error: HTTP {}", resp.status()));
                }
                let body: Value = resp.json().map_err(|e| format!("Bybit parse: {e}"))?;
                let list = body["result"]["list"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                let mut candles: Vec<Value> = list
                    .into_iter()
                    .map(|k| {
                        let arr = k.as_array().unwrap();
                        let ts_ms = arr[0].as_str().unwrap_or("0").parse::<i64>().unwrap_or(0);
                        let time = chrono::DateTime::from_timestamp_millis(ts_ms)
                            .unwrap_or(Utc::now())
                            .to_rfc3339();
                        serde_json::json!({
                            "time": time,
                            "open": arr[1].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                            "high": arr[2].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                            "low": arr[3].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                            "close": arr[4].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                            "volume": arr[5].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                        })
                    })
                    .collect();
                // Bybit returns newest-first
                candles.reverse();
                candles
            }
            "okx" => {
                let resp = client
                    .get("https://www.okx.com/api/v5/market/history-candles")
                    .query(&[
                        ("instId", symbol.as_str()),
                        ("bar", interval.as_str()),
                        ("limit", &max_per_page.to_string()),
                    ])
                    .query(&[
                        ("after", &(cursor_ms - 1).to_string()),
                        (
                            "before",
                            &(cursor_ms + max_per_page * interval_ms).to_string(),
                        ),
                    ])
                    .send()
                    .map_err(|e| format!("OKX request: {e}"))?;
                if !resp.status().is_success() {
                    return Err(format!("OKX API error: HTTP {}", resp.status()));
                }
                let body: Value = resp.json().map_err(|e| format!("OKX parse: {e}"))?;
                let data = body["data"].as_array().cloned().unwrap_or_default();
                let mut candles: Vec<Value> = data
                    .into_iter()
                    .map(|k| {
                        let arr = k.as_array().unwrap();
                        let ts_ms = arr[0].as_str().unwrap_or("0").parse::<i64>().unwrap_or(0);
                        let time = chrono::DateTime::from_timestamp_millis(ts_ms)
                            .unwrap_or(Utc::now())
                            .to_rfc3339();
                        serde_json::json!({
                            "time": time,
                            "open": arr[1].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                            "high": arr[2].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                            "low": arr[3].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                            "close": arr[4].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                            "volume": arr[5].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                        })
                    })
                    .collect();
                // OKX returns newest-first
                candles.reverse();
                candles
            }
            "kraken" => {
                let since_secs = cursor_ms / 1000;
                let resp = client
                    .get("https://api.kraken.com/0/public/OHLC")
                    .query(&[
                        ("pair", symbol.as_str()),
                        ("interval", interval.as_str()),
                        ("since", &since_secs.to_string()),
                    ])
                    .send()
                    .map_err(|e| format!("Kraken request: {e}"))?;
                if !resp.status().is_success() {
                    return Err(format!("Kraken API error: HTTP {}", resp.status()));
                }
                let body: Value = resp.json().map_err(|e| format!("Kraken parse: {e}"))?;
                if let Some(err) = body["error"].as_array() {
                    if !err.is_empty() {
                        return Err(format!("Kraken error: {:?}", err));
                    }
                }
                let result = body["result"].as_object().ok_or("Kraken: no result")?;
                // The first key that isn't "last" is the pair data
                let pair_data = result
                    .iter()
                    .find(|(k, _)| *k != "last")
                    .map(|(_, v)| v.as_array().cloned().unwrap_or_default())
                    .unwrap_or_default();
                pair_data
                    .into_iter()
                    .filter_map(|k| {
                        let arr = k.as_array()?;
                        let ts_secs = arr[0].as_i64()?;
                        if ts_secs * 1000 > end_ms {
                            return None;
                        }
                        let time = chrono::DateTime::from_timestamp(ts_secs, 0)?.to_rfc3339();
                        Some(serde_json::json!({
                            "time": time,
                            "open": arr[1].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                            "high": arr[2].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                            "low": arr[3].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                            "close": arr[4].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                            "volume": arr[6].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                        }))
                    })
                    .collect()
            }
            other => {
                return Err(format!(
                    "Exchange '{}' is not supported for historical candles. Supported: binance, bybit, okx, kraken",
                    other
                ));
            }
        };

        if raw.is_empty() {
            break;
        }

        // Advance cursor past the last candle we received
        if let Some(last) = raw.last() {
            if let Some(t) = last["time"].as_str() {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(t) {
                    cursor_ms = dt.timestamp_millis() + interval_ms;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        let fetched = raw.len();
        all_candles.extend(raw);

        // If we got fewer than a full page, we've reached the end
        if (fetched as i64) < max_per_page {
            break;
        }
    }

    if all_candles.is_empty() {
        return Err(format!(
            "No candle data returned from {} for {} ({}). Check the pair and date range.",
            provider, config.pair, config.timeframe
        ));
    }

    Ok(all_candles)
}

fn write_json_line(stdin: &Arc<Mutex<ChildStdin>>, payload: Value) -> Result<(), String> {
    let mut writer = stdin.lock().map_err(|e| format!("stdin lock: {e}"))?;
    writer
        .write_all(payload.to_string().as_bytes())
        .map_err(|e| format!("stdin write: {e}"))?;
    writer
        .write_all(b"\n")
        .map_err(|e| format!("stdin newline: {e}"))?;
    writer.flush().map_err(|e| format!("stdin flush: {e}"))
}

fn push_bot_log(app_handle: &AppHandle, level: &str, message: impl Into<String>) {
    let entry = LogEntry {
        timestamp: Utc::now().to_rfc3339(),
        level: level.to_string(),
        message: message.into(),
    };

    let _ = app_handle.emit("bot:log", &entry);

    if let Some(state) = app_handle.try_state::<AppState>() {
        if let Ok(mut logs) = state.bot_logs.lock() {
            logs.push(entry.clone());
            if logs.len() > 10_000 {
                logs.drain(0..1_000);
            }
        }
    }

    persist_bot_log_entry(&entry);
}

fn persist_bot_log_entry(entry: &LogEntry) {
    let log_dir = get_data_dir().join("logs");
    if std::fs::create_dir_all(&log_dir).is_err() {
        return;
    }

    let path = log_dir.join("bot.log");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        if let Ok(line) = serde_json::to_string(entry) {
            let _ = writeln!(file, "{line}");
        }
    }
}

fn load_persisted_bot_logs(limit: usize) -> Vec<LogEntry> {
    let path = get_data_dir().join("logs").join("bot.log");
    let content = match std::fs::read_to_string(path) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };

    let mut logs = content
        .lines()
        .filter_map(|line| serde_json::from_str::<LogEntry>(line).ok())
        .collect::<Vec<_>>();
    if logs.len() > limit {
        logs = logs.split_off(logs.len() - limit);
    }
    logs
}

fn emit_bot_error(app_handle: &AppHandle, message: impl Into<String>, details: Option<String>) {
    let message = message.into();
    push_bot_log(app_handle, "error", message.clone());

    if let Some(state) = app_handle.try_state::<AppState>() {
        if let Ok(db) = state.db.lock() {
            let _ = db.execute(
                "UPDATE bot_state SET status = 'error' WHERE id = 'singleton'",
                [],
            );
        }
    }

    let _ = app_handle.emit(
        "bot:error",
        serde_json::json!({
            "message": message,
            "details": details,
        }),
    );
}

fn emit_bot_status(
    app_handle: &AppHandle,
    status: &str,
    strategy_id: Option<String>,
    exchange_id: Option<String>,
    pair: Option<String>,
    started_at: Option<String>,
    trading_mode: &str,
) {
    let _ = app_handle.emit(
        "bot:status",
        serde_json::json!({
            "status": status,
            "strategy_id": strategy_id,
            "exchange_id": exchange_id,
            "pair": pair,
            "started_at": started_at,
            "trading_mode": trading_mode,
        }),
    );
}

fn compute_paper_equity(runtime: &LiveBotRuntime) -> f64 {
    runtime
        .open_positions
        .iter()
        .fold(runtime.balance, |equity, position| {
            if position.side == "long" {
                equity + runtime.last_price * position.quantity
            } else {
                let unrealized = (position.entry_price - runtime.last_price) * position.quantity;
                equity + position.reserved_margin + unrealized
            }
        })
}

fn runtime_balance_json(runtime: &LiveBotRuntime) -> Value {
    let quote_asset = runtime.pair.split('/').nth(1).unwrap_or("USDT").to_string();
    serde_json::json!({ quote_asset: runtime.balance })
}

fn runtime_positions_json(runtime: &LiveBotRuntime) -> Value {
    let mut positions = serde_json::Map::new();
    for position in &runtime.open_positions {
        let unrealized = if position.side == "long" {
            (runtime.last_price - position.entry_price) * position.quantity
        } else {
            (position.entry_price - runtime.last_price) * position.quantity
        };

        positions.insert(
            position.pair.clone(),
            serde_json::json!({
                "id": position.id.clone(),
                "pair": position.pair.clone(),
                "side": position.side.clone(),
                "entry_price": position.entry_price,
                "quantity": position.quantity,
                "entry_fee": position.entry_fee,
                "reserved_margin": position.reserved_margin,
                "unrealized_pnl": unrealized,
            }),
        );
    }
    Value::Object(positions)
}

fn persist_paper_trade(app_handle: &AppHandle, trade: &Trade) -> Result<(), String> {
    let state = app_handle
        .try_state::<AppState>()
        .ok_or_else(|| "App state unavailable".to_string())?;
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;

    if trade.exit_price.is_none() {
        db.execute(
            "INSERT OR REPLACE INTO trades (id, strategy_id, exchange, pair, side, entry_price, exit_price, quantity, entry_time, exit_time, pnl, pnl_pct, fee, is_backtest, backtest_id, notes, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,0,NULL,?14,?15)",
            params![
                trade.id,
                trade.strategy_id,
                trade.exchange,
                trade.pair,
                trade.side,
                trade.entry_price,
                trade.exit_price,
                trade.quantity,
                trade.entry_time,
                trade.exit_time,
                trade.pnl,
                trade.pnl_pct,
                trade.fee,
                trade.notes,
                trade.created_at,
            ],
        )
        .map_err(|e| format!("Insert paper trade: {e}"))?;
    } else {
        let updated = db
            .execute(
                "UPDATE trades SET exit_price = ?1, exit_time = ?2, pnl = ?3, pnl_pct = ?4, fee = ?5, notes = ?6 WHERE id = ?7",
                params![
                    trade.exit_price,
                    trade.exit_time,
                    trade.pnl,
                    trade.pnl_pct,
                    trade.fee,
                    trade.notes,
                    trade.id,
                ],
            )
            .map_err(|e| format!("Update paper trade: {e}"))?;

        if updated == 0 {
            db.execute(
                "INSERT INTO trades (id, strategy_id, exchange, pair, side, entry_price, exit_price, quantity, entry_time, exit_time, pnl, pnl_pct, fee, is_backtest, backtest_id, notes, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,0,NULL,?14,?15)",
                params![
                    trade.id,
                    trade.strategy_id,
                    trade.exchange,
                    trade.pair,
                    trade.side,
                    trade.entry_price,
                    trade.exit_price,
                    trade.quantity,
                    trade.entry_time,
                    trade.exit_time,
                    trade.pnl,
                    trade.pnl_pct,
                    trade.fee,
                    trade.notes,
                    trade.created_at,
                ],
            )
            .map_err(|e| format!("Insert closed paper trade: {e}"))?;
        }
    }

    Ok(())
}

fn persist_equity_snapshot(app_handle: &AppHandle, timestamp: &str, equity: f64, source: &str) {
    if let Some(state) = app_handle.try_state::<AppState>() {
        if let Ok(db) = state.db.lock() {
            let _ = db.execute(
                "INSERT INTO equity_snapshots (timestamp, equity, source) VALUES (?1, ?2, ?3)",
                params![timestamp, equity, source],
            );
        }
    }
}

fn latest_export_file() -> Result<PathBuf, String> {
    let export_dir = get_data_dir().join("exports");
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(&export_dir)
        .map_err(|e| format!("Read export dir: {e}"))?
        .filter_map(|entry| entry.ok().map(|item| item.path()))
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect();

    candidates.sort();
    candidates
        .pop()
        .ok_or_else(|| "No export snapshot found. Export data first.".to_string())
}

const EMA_CROSS_STRATEGY: &str = r#"from quantalgo import Strategy


class EMA_Cross(Strategy):
    """EMA crossover strategy — goes long when the fast EMA crosses above
    the slow EMA, goes short when it crosses below, or both.

    The ``direction`` param controls which sides are traded:
      - ``"both"``  — take both long and short entries
      - ``"long"``  — only take long entries
      - ``"short"`` — only take short entries
    """

    params = {
        "fast_period": 12,
        "slow_period": 21,
        "position_size": 0.95,
        "direction": "both",  # "both" | "long" | "short"
    }

    def on_candle(self, candle):
        history = self._candle_history
        if len(history) < self.params["slow_period"] + 1:
            return

        closes = [c.close for c in history]
        fast = self.ema(closes, self.params["fast_period"])
        slow = self.ema(closes, self.params["slow_period"])

        cur_fast, cur_slow = fast[-1], slow[-1]
        prev_fast, prev_slow = fast[-2], slow[-2]

        if None in (cur_fast, cur_slow, prev_fast, prev_slow):
            return

        pair = self._pair
        pos = self.get_position(pair)
        in_position = pos is not None
        direction = self.params["direction"]

        cross_up = prev_fast <= prev_slow and cur_fast > cur_slow
        cross_down = prev_fast >= prev_slow and cur_fast < cur_slow

        # fast crosses above slow
        if cross_up:
            # Reverse short -> long in one host-side operation so sizing uses post-close cash.
            if in_position and getattr(pos, "side", None) == "short":
                if direction in ("both", "long"):
                    self.reverse(pair, "buy", self.params["position_size"])
                else:
                    self.close()
                return
            # open long
            if not in_position and direction in ("both", "long"):
                balance = self.get_balance()
                capital = list(balance.values())[0] if isinstance(balance, dict) else (balance or 0)
                qty = (capital * self.params["position_size"]) / candle.close
                if qty > 0:
                    self.buy(pair, qty)

        # fast crosses below slow
        elif cross_down:
            # Reverse long -> short in one host-side operation so sizing uses post-close cash.
            if in_position and getattr(pos, "side", None) == "long":
                if direction in ("both", "short"):
                    self.reverse(pair, "sell", self.params["position_size"])
                else:
                    self.close()
                return
            # open short
            if not in_position and direction in ("both", "short"):
                balance = self.get_balance()
                capital = list(balance.values())[0] if isinstance(balance, dict) else (balance or 0)
                qty = (capital * self.params["position_size"]) / candle.close
                if qty > 0:
                    self.sell(pair, qty)
"#;

const STRATEGY_TEMPLATE: &str = r#"from quantalgo import Strategy


class NewStrategy(Strategy):
    """Strategy description here."""

    params = {
        "fast_period": 12,
        "slow_period": 26,
        "position_size": 0.95,
    }

    def on_candle(self, candle):
        pass

    def on_tick(self, tick):
        pass

    def on_trade(self, trade):
        pass

    def on_start(self):
        pass

    def on_stop(self):
        pass
"#;

// ---------------------------------------------------------------------------
// Database
// ---------------------------------------------------------------------------

pub fn init_db(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS trades (
            id TEXT PRIMARY KEY,
            strategy_id TEXT NOT NULL,
            exchange TEXT NOT NULL,
            pair TEXT NOT NULL,
            side TEXT NOT NULL,
            entry_price REAL NOT NULL,
            exit_price REAL,
            quantity REAL NOT NULL,
            entry_time TEXT NOT NULL,
            exit_time TEXT,
            pnl REAL,
            pnl_pct REAL,
            fee REAL DEFAULT 0,
            is_backtest INTEGER DEFAULT 0,
            backtest_id TEXT,
            notes TEXT,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS strategies (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            file_path TEXT NOT NULL,
            params_json TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS backtests (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            strategy_id TEXT NOT NULL,
            config_json TEXT NOT NULL,
            stats_json TEXT NOT NULL,
            equity_curve_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS exchanges (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            exchange_type TEXT NOT NULL,
            provider TEXT NOT NULL,
            config_encrypted TEXT,
            is_active INTEGER DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS bot_state (
            id TEXT PRIMARY KEY DEFAULT 'singleton',
            status TEXT DEFAULT 'stopped',
            strategy_id TEXT,
            exchange_id TEXT,
            pair TEXT,
            started_at TEXT,
            config_json TEXT,
            trading_mode TEXT DEFAULT 'paper'
        );

        CREATE TABLE IF NOT EXISTS equity_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            equity REAL NOT NULL,
            source TEXT DEFAULT 'paper'
        );

        INSERT OR IGNORE INTO bot_state (id, status) VALUES ('singleton', 'stopped');

        CREATE TABLE IF NOT EXISTS migrations (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL
        );
        ",
    )?;
    Ok(())
}

/// Run pending data migrations. Each migration only runs once.
fn run_migrations(conn: &Connection, strategy_dir: &std::path::Path) -> Result<(), String> {
    let applied: Vec<i64> = conn
        .prepare("SELECT id FROM migrations")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get(0))?
                .collect::<Result<Vec<i64>, _>>()
        })
        .unwrap_or_default();

    let now = chrono::Utc::now().to_rfc3339();

    // Migration 1: Update EMA Cross strategy to support direction (long/short/both)
    if !applied.contains(&1) {
        let updated = migrate_ema_cross_direction(conn, strategy_dir);
        if let Err(e) = &updated {
            eprintln!("[quantalgo] migration 1 (ema direction) failed: {e}");
        }
        // Mark applied even on soft failure so we don't retry endlessly
        let _ = conn.execute(
            "INSERT INTO migrations (id, name, applied_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![1, "ema_cross_direction", now],
        );
    }

    // Migration 2: Add trading_mode column to bot_state
    if !applied.contains(&2) {
        let has_col = conn
            .prepare("SELECT trading_mode FROM bot_state LIMIT 0")
            .is_ok();
        if !has_col {
            let _ = conn.execute(
                "ALTER TABLE bot_state ADD COLUMN trading_mode TEXT DEFAULT 'paper'",
                [],
            );
        }
        let _ = conn.execute(
            "INSERT INTO migrations (id, name, applied_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![2, "bot_state_trading_mode", now],
        );
    }

    // Migration 3: Synthetic runtime snapshots were previously mislabeled as live.
    if !applied.contains(&3) {
        let _ = conn.execute(
            "UPDATE equity_snapshots SET source = 'paper' WHERE source = 'live'",
            [],
        );
        let _ = conn.execute(
            "INSERT INTO migrations (id, name, applied_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![3, "paper_equity_source", now],
        );
    }

    // Migration 4: Update EMA Cross flips to use host-side reverse sizing.
    if !applied.contains(&4) {
        let updated = migrate_ema_cross_atomic_reverse(conn, strategy_dir);
        if let Err(e) = &updated {
            eprintln!("[quantalgo] migration 4 (ema atomic reverse) failed: {e}");
        }
        let _ = conn.execute(
            "INSERT INTO migrations (id, name, applied_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![4, "ema_cross_atomic_reverse", now],
        );
    }

    Ok(())
}

/// Migration 1: update existing EMA Cross strategy file + params to include direction.
fn migrate_ema_cross_direction(
    conn: &Connection,
    _strategy_dir: &std::path::Path,
) -> Result<(), String> {
    // Find the EMA Cross strategy by name
    let row: Option<(String, String, Option<String>)> = conn
        .query_row(
            "SELECT id, file_path, params_json FROM strategies WHERE name = 'EMA Cross' LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .ok();

    let (id, file_path, params_json) = match row {
        Some(r) => r,
        None => return Ok(()), // no EMA Cross strategy, nothing to migrate
    };

    // Update the strategy file on disk
    std::fs::write(&file_path, EMA_CROSS_STRATEGY)
        .map_err(|e| format!("Write strategy file: {e}"))?;

    // Merge direction into existing params (preserve user's custom values)
    let mut params: serde_json::Value = params_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    if let Some(obj) = params.as_object_mut() {
        obj.entry("direction".to_string())
            .or_insert(serde_json::json!("both"));
    }

    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE strategies SET params_json = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![serde_json::to_string(&params).unwrap_or_default(), now, id],
    )
    .map_err(|e| format!("Update params: {e}"))?;

    Ok(())
}

/// Migration 4: update the built-in EMA Cross strategy so reversals are atomic.
fn migrate_ema_cross_atomic_reverse(
    conn: &Connection,
    _strategy_dir: &std::path::Path,
) -> Result<(), String> {
    let row: Option<(String, String, Option<String>)> = conn
        .query_row(
            "SELECT id, file_path, params_json FROM strategies WHERE name = 'EMA Cross' LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .ok();

    let (id, file_path, params_json) = match row {
        Some(r) => r,
        None => return Ok(()),
    };

    let current_code = std::fs::read_to_string(&file_path).unwrap_or_default();
    let has_stale_close_then_size = current_code.contains("self.close()")
        && current_code.contains("qty = (capital * self.params[\"position_size\"]) / candle.close")
        && !current_code.contains("self.reverse(");

    if current_code.is_empty() || has_stale_close_then_size {
        std::fs::write(&file_path, EMA_CROSS_STRATEGY)
            .map_err(|e| format!("Write atomic reverse strategy file: {e}"))?;
    }

    let mut params: serde_json::Value = params_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    if let Some(obj) = params.as_object_mut() {
        obj.entry("direction".to_string())
            .or_insert(serde_json::json!("both"));
    }

    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE strategies SET params_json = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![serde_json::to_string(&params).unwrap_or_default(), now, id],
    )
    .map_err(|e| format!("Update params: {e}"))?;

    Ok(())
}

/// Seed the built-in EMA Cross test strategy on first run (no strategies yet).
fn seed_strategies(conn: &Connection, strategy_dir: &std::path::Path) -> Result<(), String> {
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM strategies", [], |row| row.get(0))
        .map_err(|e| format!("Count strategies: {e}"))?;
    if count > 0 {
        return Ok(());
    }

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let file_name = format!("EMA_Cross_{}.py", &id[..8]);
    let file_path = strategy_dir.join(&file_name);

    std::fs::write(&file_path, EMA_CROSS_STRATEGY)
        .map_err(|e| format!("Write seed strategy: {e}"))?;

    let params_json =
        r#"{"fast_period":12,"slow_period":21,"position_size":0.95,"direction":"both"}"#;
    conn.execute(
        "INSERT INTO strategies (id, name, description, file_path, params_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            id,
            "EMA Cross",
            "EMA crossover (12/21) — long, short, or both on golden/death cross",
            file_path.to_string_lossy().to_string(),
            params_json,
            now,
            now,
        ],
    )
    .map_err(|e| format!("Insert seed strategy: {e}"))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Strategy Commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn list_strategies(state: State<'_, AppState>) -> Result<Vec<Strategy>, String> {
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    let mut stmt = db
        .prepare("SELECT id, name, description, file_path, params_json, created_at, updated_at FROM strategies ORDER BY updated_at DESC")
        .map_err(|e| format!("Prepare: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Strategy {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                file_path: row.get(3)?,
                params_json: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .map_err(|e| format!("Query: {e}"))?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| format!("Row: {e}"))?);
    }
    Ok(result)
}

#[tauri::command]
fn get_strategy(id: String, state: State<'_, AppState>) -> Result<Strategy, String> {
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    db.query_row(
        "SELECT id, name, description, file_path, params_json, created_at, updated_at FROM strategies WHERE id = ?1",
        params![id],
        |row| {
            Ok(Strategy {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                file_path: row.get(3)?,
                params_json: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        },
    )
    .map_err(|e| format!("Not found: {e}"))
}

#[tauri::command]
fn create_strategy(
    name: String,
    description: String,
    state: State<'_, AppState>,
) -> Result<Strategy, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let settings = state.settings.lock().map_err(|e| format!("Lock: {e}"))?;
    let strategy_dir = PathBuf::from(&settings.strategy_dir);
    drop(settings);

    std::fs::create_dir_all(&strategy_dir).map_err(|e| format!("Mkdir: {e}"))?;

    let safe_name: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let file_name = format!("{safe_name}_{}.py", &id[..8]);
    let file_path = strategy_dir.join(&file_name);

    let template = STRATEGY_TEMPLATE.replace("NewStrategy", &safe_name);
    let template = template.replace("Strategy description here.", &description);
    std::fs::write(&file_path, &template).map_err(|e| format!("Write file: {e}"))?;

    let file_path_str = file_path.to_string_lossy().to_string();
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    db.execute(
        "INSERT INTO strategies (id, name, description, file_path, params_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, name, description, file_path_str, None::<String>, now, now],
    )
    .map_err(|e| format!("Insert: {e}"))?;

    Ok(Strategy {
        id,
        name,
        description,
        file_path: file_path_str,
        params_json: None,
        created_at: now.clone(),
        updated_at: now,
    })
}

#[tauri::command]
fn save_strategy(
    id: String,
    code: String,
    params: Option<Value>,
    state: State<'_, AppState>,
) -> Result<Strategy, String> {
    let now = Utc::now().to_rfc3339();
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;

    let strategy: Strategy = db
        .query_row(
            "SELECT id, name, description, file_path, params_json, created_at, updated_at FROM strategies WHERE id = ?1",
            params![id],
            |row| {
                Ok(Strategy {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    file_path: row.get(3)?,
                    params_json: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        )
        .map_err(|e| format!("Not found: {e}"))?;

    std::fs::write(&strategy.file_path, &code).map_err(|e| format!("Write file: {e}"))?;

    let params_json = params.map(|v| serde_json::to_string(&v).unwrap_or_default());

    db.execute(
        "UPDATE strategies SET params_json = ?1, updated_at = ?2 WHERE id = ?3",
        params![params_json, now, id],
    )
    .map_err(|e| format!("Update: {e}"))?;

    Ok(Strategy {
        id: strategy.id,
        name: strategy.name,
        description: strategy.description,
        file_path: strategy.file_path,
        params_json,
        created_at: strategy.created_at,
        updated_at: now,
    })
}

#[tauri::command]
fn update_strategy_meta(
    id: String,
    name: Option<String>,
    description: Option<String>,
    state: State<'_, AppState>,
) -> Result<Strategy, String> {
    let now = Utc::now().to_rfc3339();
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;

    let mut strategy: Strategy = db
        .query_row(
            "SELECT id, name, description, file_path, params_json, created_at, updated_at FROM strategies WHERE id = ?1",
            params![id],
            |row| {
                Ok(Strategy {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    file_path: row.get(3)?,
                    params_json: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        )
        .map_err(|e| format!("Not found: {e}"))?;

    if let Some(value) = name {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            strategy.name = trimmed.to_string();
        }
    }

    if let Some(value) = description {
        strategy.description = value.trim().to_string();
    }

    db.execute(
        "UPDATE strategies SET name = ?1, description = ?2, updated_at = ?3 WHERE id = ?4",
        params![strategy.name, strategy.description, now, strategy.id],
    )
    .map_err(|e| format!("Update: {e}"))?;

    strategy.updated_at = now;
    Ok(strategy)
}

#[tauri::command]
fn delete_strategy(id: String, state: State<'_, AppState>) -> Result<bool, String> {
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;

    let file_path: Option<String> = db
        .query_row(
            "SELECT file_path FROM strategies WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .ok();

    if let Some(ref path) = file_path {
        let _ = std::fs::remove_file(path);
    }

    let affected = db
        .execute("DELETE FROM strategies WHERE id = ?1", params![id])
        .map_err(|e| format!("Delete: {e}"))?;

    Ok(affected > 0)
}

#[tauri::command]
fn read_strategy_file(id: String, state: State<'_, AppState>) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    let file_path: String = db
        .query_row(
            "SELECT file_path FROM strategies WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Not found: {e}"))?;
    std::fs::read_to_string(&file_path).map_err(|e| format!("Read: {e}"))
}

// ---------------------------------------------------------------------------
// Backtest Commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn run_backtest(
    config: BacktestConfig,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<BacktestResult, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let _ = app_handle.emit(
        "backtest:progress",
        serde_json::json!({ "pct": 5.0, "message": "Preparing backtest environment..." }),
    );

    let (strategy_file_path, strategy_params_json) = {
        let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
        db.query_row(
            "SELECT file_path, params_json FROM strategies WHERE id = ?1",
            params![config.strategy_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .map_err(|e| format!("Strategy lookup failed: {e}"))?
    };

    let settings = state.settings.lock().map_err(|e| format!("Lock: {e}"))?;
    let python_path = resolve_python_path(&settings);
    let slippage_pct = settings.slippage_tolerance;
    drop(settings);

    let _ = app_handle.emit(
        "backtest:progress",
        serde_json::json!({ "pct": 10.0, "message": format!("Fetching {} candles from {}...", config.pair, config.exchange) }),
    );
    let candles = fetch_historical_candles(&config)?;
    let candle_times: Vec<String> = candles
        .iter()
        .filter_map(|candle| {
            candle
                .get("time")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
        })
        .collect();

    let mut strategy_params = strategy_params_json
        .as_deref()
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    // Merge UI-provided param overrides (e.g. direction dropdown)
    if let Some(overrides) = &config.strategy_params {
        if let (Some(base), Some(ovr)) = (strategy_params.as_object_mut(), overrides.as_object()) {
            for (k, v) in ovr {
                base.insert(k.clone(), v.clone());
            }
        }
    }

    let payload = serde_json::json!({
        "config": {
            "initial_balance": config.initial_capital,
            "commission_pct": config.commission,
            "slippage_pct": slippage_pct,
            "base_asset": config.pair.split('/').nth(1).unwrap_or("USDT"),
            "pair": config.pair,
        },
        "params": strategy_params,
        "candles": candles,
    });

    let _ = app_handle.emit(
        "backtest:progress",
        serde_json::json!({ "pct": 20.0, "message": format!("Running {} candles through Python strategy...", candle_times.len()) }),
    );

    let mut command = build_python_command(&python_path);
    command
        .arg("-m")
        .arg("quantalgo.runner")
        .arg(&strategy_file_path)
        .arg("--backtest")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().map_err(|e| {
        let message = format!("Failed to spawn strategy runner: {e}");
        emit_bot_error(&app_handle, message.clone(), None);
        message
    })?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(payload.to_string().as_bytes())
            .and_then(|_| stdin.flush())
            .map_err(|e| format!("Write stdin: {e}"))?;
    } else {
        return Err("Backtest runner stdin unavailable".into());
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Wait for backtest: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !stderr.trim().is_empty() {
        let _ = app_handle.emit(
            "backtest:progress",
            serde_json::json!({ "pct": 70.0, "message": stderr.lines().last().unwrap_or("Backtest finished with runner output.") }),
        );
    }

    if !output.status.success() {
        return Err(format!(
            "Backtest runner failed with status {}{}",
            output.status,
            if stderr.trim().is_empty() {
                String::new()
            } else {
                format!(": {}", stderr.trim())
            }
        ));
    }

    let (stats, equity_curve, trades) = parse_backtest_output(
        &stdout,
        &config,
        &id,
        &config.strategy_id,
        &now,
        &candle_times,
    )?;

    let result = BacktestResult {
        id: id.clone(),
        name: format!(
            "Backtest {} {}",
            config.pair,
            Utc::now().format("%Y-%m-%d %H:%M")
        ),
        strategy_id: config.strategy_id.clone(),
        config,
        stats,
        equity_curve,
        trades,
        created_at: now,
    };

    let _ = app_handle.emit(
        "backtest:progress",
        serde_json::json!({ "pct": 100.0, "message": "Backtest complete." }),
    );
    let _ = app_handle.emit(
        "backtest:complete",
        serde_json::json!({ "result": &result }),
    );

    Ok(result)
}

fn parse_backtest_output(
    output: &str,
    config: &BacktestConfig,
    backtest_id: &str,
    strategy_id: &str,
    created_at: &str,
    candle_times: &[String],
) -> Result<(BacktestStats, Vec<EquityPoint>, Vec<Trade>), String> {
    #[derive(Debug, Deserialize)]
    struct PythonTrade {
        id: String,
        pair: String,
        side: String,
        entry_price: f64,
        exit_price: f64,
        quantity: f64,
        pnl: f64,
        entry_time: String,
        exit_time: String,
        #[serde(default)]
        commission: f64,
    }

    #[derive(Debug, Deserialize)]
    struct PythonBacktestOutput {
        #[serde(default)]
        initial_balance: f64,
        #[serde(default)]
        final_balance: f64,
        #[serde(default)]
        equity_curve: Vec<f64>,
        #[serde(default)]
        trades: Vec<PythonTrade>,
        #[serde(default)]
        stats: Value,
    }

    for line in output.lines().rev() {
        let trimmed = line.trim();
        if trimmed.starts_with('{') {
            if let Ok(parsed) = serde_json::from_str::<PythonBacktestOutput>(trimmed) {
                let gross_profit = parsed
                    .stats
                    .get("gross_profit")
                    .and_then(|value| value.as_f64())
                    .unwrap_or(0.0);
                let gross_loss = parsed
                    .stats
                    .get("gross_loss")
                    .and_then(|value| value.as_f64())
                    .unwrap_or(0.0);
                let profit_factor = match parsed.stats.get("profit_factor") {
                    Some(Value::Number(value)) => value.as_f64().unwrap_or(0.0),
                    Some(Value::String(value)) if value.eq_ignore_ascii_case("infinity") => {
                        gross_profit.max(1.0)
                    }
                    _ => 0.0,
                };

                let stats = BacktestStats {
                    total_return: parsed.final_balance - parsed.initial_balance,
                    total_return_pct: parsed
                        .stats
                        .get("total_return_pct")
                        .and_then(|value| value.as_f64())
                        .unwrap_or(0.0),
                    sharpe_ratio: parsed
                        .stats
                        .get("sharpe_ratio")
                        .and_then(|value| value.as_f64())
                        .unwrap_or(0.0),
                    max_drawdown: config.initial_capital
                        * parsed
                            .stats
                            .get("max_drawdown_pct")
                            .and_then(|value| value.as_f64())
                            .unwrap_or(0.0)
                        / 100.0,
                    max_drawdown_pct: parsed
                        .stats
                        .get("max_drawdown_pct")
                        .and_then(|value| value.as_f64())
                        .unwrap_or(0.0),
                    win_rate: parsed
                        .stats
                        .get("win_rate_pct")
                        .and_then(|value| value.as_f64())
                        .unwrap_or(0.0),
                    profit_factor,
                    total_trades: parsed
                        .stats
                        .get("total_trades")
                        .and_then(|value| value.as_i64())
                        .unwrap_or(parsed.trades.len() as i64),
                    avg_trade_duration_secs: parsed
                        .stats
                        .get("avg_trade_duration_seconds")
                        .and_then(|value| value.as_f64())
                        .unwrap_or(0.0),
                };

                let equity_curve = parsed
                    .equity_curve
                    .iter()
                    .enumerate()
                    .map(|(idx, equity)| EquityPoint {
                        time: candle_times
                            .get(idx)
                            .cloned()
                            .unwrap_or_else(|| config.start_date.clone()),
                        equity: *equity,
                    })
                    .collect::<Vec<_>>();

                let trades = parsed
                    .trades
                    .into_iter()
                    .map(|trade| {
                        let notional = trade.entry_price * trade.quantity;
                        let pnl_pct = if notional > 0.0 {
                            (trade.pnl / notional) * 100.0
                        } else {
                            0.0
                        };

                        Trade {
                            id: trade.id,
                            strategy_id: strategy_id.to_string(),
                            exchange: config.exchange.clone(),
                            pair: trade.pair,
                            side: trade.side,
                            entry_price: trade.entry_price,
                            exit_price: Some(trade.exit_price),
                            quantity: trade.quantity,
                            entry_time: trade.entry_time,
                            exit_time: Some(trade.exit_time),
                            pnl: Some(trade.pnl),
                            pnl_pct: Some(pnl_pct),
                            fee: trade.commission,
                            is_backtest: true,
                            backtest_id: Some(backtest_id.to_string()),
                            notes: None,
                            created_at: created_at.to_string(),
                        }
                    })
                    .collect::<Vec<_>>();

                let _ = gross_loss;
                return Ok((stats, equity_curve, trades));
            }
        }
    }

    Err("Backtest runner returned no parseable result.".into())
}

#[tauri::command]
fn list_backtests(state: State<'_, AppState>) -> Result<Vec<BacktestSummary>, String> {
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    let mut stmt = db
        .prepare("SELECT id, name, strategy_id, config_json, stats_json, created_at FROM backtests ORDER BY created_at DESC")
        .map_err(|e| format!("Prepare: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(BacktestSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                strategy_id: row.get(2)?,
                config_json: row.get(3)?,
                stats_json: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| format!("Query: {e}"))?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| format!("Row: {e}"))?);
    }
    Ok(result)
}

#[tauri::command]
fn get_backtest(id: String, state: State<'_, AppState>) -> Result<BacktestResult, String> {
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    let (name, strategy_id, config_json, stats_json, eq_json, created_at): (
        String,
        String,
        String,
        String,
        String,
        String,
    ) = db
        .query_row(
            "SELECT name, strategy_id, config_json, stats_json, equity_curve_json, created_at FROM backtests WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        )
        .map_err(|e| format!("Not found: {e}"))?;

    let config: BacktestConfig =
        serde_json::from_str(&config_json).map_err(|e| format!("Parse config: {e}"))?;
    let stats: BacktestStats =
        serde_json::from_str(&stats_json).map_err(|e| format!("Parse stats: {e}"))?;

    #[derive(Deserialize)]
    struct EqData {
        equity_curve: Vec<EquityPoint>,
        trades: Vec<Trade>,
    }
    let eq_data: EqData = serde_json::from_str(&eq_json).unwrap_or(EqData {
        equity_curve: Vec::new(),
        trades: Vec::new(),
    });

    Ok(BacktestResult {
        id,
        name,
        strategy_id,
        config,
        stats,
        equity_curve: eq_data.equity_curve,
        trades: eq_data.trades,
        created_at,
    })
}

#[tauri::command]
fn save_backtest(
    result: BacktestResult,
    name: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    let config_json =
        serde_json::to_string(&result.config).map_err(|e| format!("Serialize: {e}"))?;
    let stats_json = serde_json::to_string(&result.stats).map_err(|e| format!("Serialize: {e}"))?;
    let eq_json = serde_json::to_string(&serde_json::json!({
        "equity_curve": result.equity_curve,
        "trades": result.trades,
    }))
    .map_err(|e| format!("Serialize: {e}"))?;

    db.execute(
        "INSERT OR REPLACE INTO backtests (id, name, strategy_id, config_json, stats_json, equity_curve_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![result.id, name, result.strategy_id, config_json, stats_json, eq_json, result.created_at],
    )
    .map_err(|e| format!("Insert: {e}"))?;

    // Also persist backtest trades into the trades table
    for trade in &result.trades {
        db.execute(
            "INSERT OR IGNORE INTO trades (id, strategy_id, exchange, pair, side, entry_price, exit_price, quantity, entry_time, exit_time, pnl, pnl_pct, fee, is_backtest, backtest_id, notes, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,1,?14,?15,?16)",
            params![
                trade.id,
                trade.strategy_id,
                trade.exchange,
                trade.pair,
                trade.side,
                trade.entry_price,
                trade.exit_price,
                trade.quantity,
                trade.entry_time,
                trade.exit_time,
                trade.pnl,
                trade.pnl_pct,
                trade.fee,
                trade.backtest_id,
                trade.notes,
                trade.created_at,
            ],
        )
        .map_err(|e| format!("Insert trade: {e}"))?;
    }

    Ok(result.id)
}

#[tauri::command]
fn delete_backtest(id: String, state: State<'_, AppState>) -> Result<bool, String> {
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    db.execute("DELETE FROM trades WHERE backtest_id = ?1", params![id])
        .map_err(|e| format!("Delete trades: {e}"))?;
    let affected = db
        .execute("DELETE FROM backtests WHERE id = ?1", params![id])
        .map_err(|e| format!("Delete: {e}"))?;
    Ok(affected > 0)
}

// ---------------------------------------------------------------------------
// Bot Commands
// ---------------------------------------------------------------------------

fn emit_paper_trade(app_handle: &AppHandle, trade: &Trade) {
    let _ = persist_paper_trade(app_handle, trade);
    let _ = app_handle.emit("bot:trade", serde_json::json!({ "trade": trade }));
}

fn emit_paper_equity(app_handle: &AppHandle, runtime: &LiveBotRuntime) {
    let timestamp = Utc::now().to_rfc3339();
    let equity = compute_paper_equity(runtime);
    persist_equity_snapshot(app_handle, &timestamp, equity, "paper");
    let _ = app_handle.emit(
        "bot:equity",
        serde_json::json!({
            "timestamp": timestamp,
            "equity": equity,
            "last_price": runtime.last_price,
            "pair": runtime.pair,
            "balance": runtime.balance,
            "open_position_count": runtime.open_positions.len(),
        }),
    );
}

fn notify_strategy_trade(
    stdin: &Arc<Mutex<ChildStdin>>,
    trade_id: &str,
    pair: &str,
    side: &str,
    price: f64,
    quantity: f64,
    pnl: f64,
    action: &str,
    runtime: &LiveBotRuntime,
) {
    let _ = write_json_line(
        stdin,
        serde_json::json!({
            "method": "on_trade",
            "params": {
                "id": trade_id,
                "pair": pair,
                "side": side,
                "price": price,
                "quantity": quantity,
                "time": Utc::now().to_rfc3339(),
                "pnl": pnl,
                "action": action,
                "balance": runtime_balance_json(runtime),
                "positions": runtime_positions_json(runtime),
                "mark_price": runtime.last_price,
            }
        }),
    );
}

fn handle_strategy_rpc_line(
    app_handle: &AppHandle,
    stdin: &Arc<Mutex<ChildStdin>>,
    runtime: &Arc<Mutex<LiveBotRuntime>>,
    line: &str,
) {
    let parsed = match serde_json::from_str::<Value>(line) {
        Ok(value) => value,
        Err(_) => {
            push_bot_log(app_handle, "info", line.to_string());
            return;
        }
    };

    let method = parsed
        .get("method")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let params = parsed.get("params").cloned().unwrap_or(Value::Null);

    match method {
        "log" => {
            let level = params
                .get("level")
                .and_then(|value| value.as_str())
                .unwrap_or("info");
            let message = params
                .get("message")
                .and_then(|value| value.as_str())
                .unwrap_or("Strategy log")
                .to_string();
            push_bot_log(app_handle, level, message);
        }
        "buy" | "sell" | "reverse" => {
            let order_method = if method == "reverse" {
                match params
                    .get("side")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                {
                    "buy" | "long" => "buy",
                    "sell" | "short" => "sell",
                    other => {
                        push_bot_log(app_handle, "warn", format!("Invalid reverse side: {other}"));
                        return;
                    }
                }
            } else {
                method
            };
            let reverse_after_close = method == "reverse"
                || params
                    .get("reverse")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false);
            let pair = params
                .get("pair")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
                .unwrap_or_else(|| {
                    runtime
                        .lock()
                        .map(|value| value.pair.clone())
                        .unwrap_or_else(|_| "BTC/USDT".into())
                });
            let requested_qty = params
                .get("quantity")
                .and_then(|value| value.as_f64())
                .unwrap_or(0.0);
            let position_size = params
                .get("position_size")
                .or_else(|| params.get("sizing_pct"))
                .and_then(|value| value.as_f64())
                .map(|value| if value > 1.0 { value / 100.0 } else { value })
                .filter(|value| *value > 0.0 && *value <= 1.0);

            let mut runtime = match runtime.lock() {
                Ok(value) => value,
                Err(err) => {
                    push_bot_log(
                        app_handle,
                        "error",
                        format!("Bot runtime lock failed: {err}"),
                    );
                    return;
                }
            };

            // Enforce max positions limit
            let opening_side = if order_method == "buy" {
                "long"
            } else {
                "short"
            };
            let opposing_side = if order_method == "buy" {
                "short"
            } else {
                "long"
            };
            let has_opposing = runtime
                .open_positions
                .iter()
                .any(|p| p.pair == pair && p.side == opposing_side);
            let has_same_side = runtime
                .open_positions
                .iter()
                .any(|p| p.pair == pair && p.side == opening_side);
            if !has_opposing && has_same_side {
                push_bot_log(
                    app_handle,
                    "warn",
                    format!(
                        "One open {opening_side} position per pair is supported; skipping duplicate {pair} signal."
                    ),
                );
                return;
            }
            if !has_opposing && runtime.open_positions.len() >= runtime.max_positions {
                push_bot_log(
                    app_handle,
                    "warn",
                    format!(
                        "Max positions ({}) reached, skipping {} signal.",
                        runtime.max_positions, order_method
                    ),
                );
                return;
            }

            let fee_rate = runtime.fee_rate;
            // Apply slippage: buys fill slightly higher, sells slightly lower
            let slippage_mult = if order_method == "buy" {
                1.0 + runtime.slippage_pct / 100.0
            } else {
                1.0 - runtime.slippage_pct / 100.0
            };
            let price = (runtime.last_price * slippage_mult).max(0.0001);

            let timestamp = Utc::now().to_rfc3339();
            let mut closed_opposing = false;

            if let Some(position_idx) = runtime
                .open_positions
                .iter()
                .position(|position| position.pair == pair && position.side == opposing_side)
            {
                let position = runtime.open_positions.remove(position_idx);
                let exit_fee = price * position.quantity * fee_rate;
                let total_fee = position.entry_fee + exit_fee;
                let pnl = if position.side == "long" {
                    (price - position.entry_price) * position.quantity - total_fee
                } else {
                    (position.entry_price - price) * position.quantity - total_fee
                };

                if position.side == "long" {
                    runtime.balance += price * position.quantity - exit_fee;
                } else {
                    runtime.balance += position.reserved_margin
                        + (position.entry_price - price) * position.quantity
                        - exit_fee;
                }

                let closed_trade = Trade {
                    id: position.id.clone(),
                    strategy_id: runtime.strategy_id.clone(),
                    exchange: runtime.exchange_id.clone(),
                    pair: position.pair.clone(),
                    side: position.side.clone(),
                    entry_price: position.entry_price,
                    exit_price: Some(price),
                    quantity: position.quantity,
                    entry_time: position.entry_time.clone(),
                    exit_time: Some(timestamp.clone()),
                    pnl: Some(pnl),
                    pnl_pct: Some(
                        (pnl / (position.entry_price * position.quantity).max(0.0001)) * 100.0,
                    ),
                    fee: total_fee,
                    is_backtest: false,
                    backtest_id: None,
                    notes: None,
                    created_at: position.entry_time.clone(),
                };

                emit_paper_trade(app_handle, &closed_trade);
                notify_strategy_trade(
                    stdin,
                    &position.id,
                    &position.pair,
                    &position.side,
                    price,
                    position.quantity,
                    pnl,
                    "close",
                    &runtime,
                );
                push_bot_log(
                    app_handle,
                    "trade",
                    format!(
                        "Closed {} {} at {:.4} (PnL {:+.2})",
                        position.side, position.pair, price, pnl
                    ),
                );
                closed_opposing = true;
            }

            if closed_opposing && !reverse_after_close {
                emit_paper_equity(app_handle, &runtime);
                return;
            }

            if runtime.open_positions.len() >= runtime.max_positions {
                push_bot_log(
                    app_handle,
                    "warn",
                    format!(
                        "Max positions ({}) reached after close, skipping {} entry.",
                        runtime.max_positions, order_method
                    ),
                );
                emit_paper_equity(app_handle, &runtime);
                return;
            }

            // Position sizing: explicit quantity wins. Otherwise, use the
            // strategy-provided fraction after any close has updated balance.
            let quantity = if requested_qty > 0.0 {
                requested_qty
            } else if let Some(fraction) = position_size {
                ((runtime.balance * fraction) / price).max(0.001)
            } else {
                ((runtime.balance * runtime.risk_per_trade / 100.0) / price).max(0.001)
            };

            let entry_fee = price * quantity * fee_rate;
            let notional = price * quantity;
            if opening_side == "long" {
                let required = notional + entry_fee;
                if runtime.balance < required {
                    push_bot_log(
                        app_handle,
                        "warn",
                        "Insufficient balance to open long position.",
                    );
                    emit_paper_equity(app_handle, &runtime);
                    return;
                }
                runtime.balance -= required;
            } else {
                let required = notional + entry_fee;
                if runtime.balance < required {
                    push_bot_log(
                        app_handle,
                        "warn",
                        "Insufficient collateral to open short position.",
                    );
                    emit_paper_equity(app_handle, &runtime);
                    return;
                }
                runtime.balance -= required;
            }

            let position_id = Uuid::new_v4().to_string();
            runtime.open_positions.push(LivePosition {
                id: position_id.clone(),
                pair: pair.clone(),
                side: opening_side.to_string(),
                entry_price: price,
                quantity,
                entry_fee,
                reserved_margin: if opening_side == "short" {
                    notional
                } else {
                    0.0
                },
                entry_time: timestamp.clone(),
            });

            let opened_trade = Trade {
                id: position_id.clone(),
                strategy_id: runtime.strategy_id.clone(),
                exchange: runtime.exchange_id.clone(),
                pair: pair.clone(),
                side: opening_side.to_string(),
                entry_price: price,
                exit_price: None,
                quantity,
                entry_time: timestamp.clone(),
                exit_time: None,
                pnl: None,
                pnl_pct: None,
                fee: entry_fee,
                is_backtest: false,
                backtest_id: None,
                notes: None,
                created_at: timestamp.clone(),
            };

            emit_paper_trade(app_handle, &opened_trade);
            notify_strategy_trade(
                stdin,
                &position_id,
                &pair,
                opening_side,
                price,
                quantity,
                0.0,
                "open",
                &runtime,
            );
            push_bot_log(
                app_handle,
                "trade",
                format!(
                    "Opened {} {} at {:.4} (qty {:.6})",
                    opening_side, pair, price, quantity
                ),
            );

            emit_paper_equity(app_handle, &runtime);
        }
        "close" => {
            let position_id = params
                .get("position_id")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string());
            let mut runtime = match runtime.lock() {
                Ok(value) => value,
                Err(err) => {
                    push_bot_log(
                        app_handle,
                        "error",
                        format!("Bot runtime lock failed: {err}"),
                    );
                    return;
                }
            };

            let fee_rate = runtime.fee_rate;

            let indices = runtime
                .open_positions
                .iter()
                .enumerate()
                .filter_map(|(idx, position)| {
                    if position_id
                        .as_ref()
                        .map(|id| id == &position.id)
                        .unwrap_or(true)
                    {
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            for idx in indices.into_iter().rev() {
                let position = runtime.open_positions.remove(idx);
                let timestamp = Utc::now().to_rfc3339();
                let close_slippage_mult = if position.side == "long" {
                    1.0 - runtime.slippage_pct / 100.0
                } else {
                    1.0 + runtime.slippage_pct / 100.0
                };
                let close_price = (runtime.last_price * close_slippage_mult).max(0.0001);
                let exit_fee = close_price * position.quantity * fee_rate;
                let total_fee = position.entry_fee + exit_fee;
                let pnl = if position.side == "long" {
                    (close_price - position.entry_price) * position.quantity - total_fee
                } else {
                    (position.entry_price - close_price) * position.quantity - total_fee
                };

                if position.side == "long" {
                    runtime.balance += close_price * position.quantity - exit_fee;
                } else {
                    runtime.balance += position.reserved_margin
                        + (position.entry_price - close_price) * position.quantity
                        - exit_fee;
                }

                let closed_trade = Trade {
                    id: position.id.clone(),
                    strategy_id: runtime.strategy_id.clone(),
                    exchange: runtime.exchange_id.clone(),
                    pair: position.pair.clone(),
                    side: position.side.clone(),
                    entry_price: position.entry_price,
                    exit_price: Some(close_price),
                    quantity: position.quantity,
                    entry_time: position.entry_time.clone(),
                    exit_time: Some(timestamp.clone()),
                    pnl: Some(pnl),
                    pnl_pct: Some(
                        (pnl / (position.entry_price * position.quantity).max(0.0001)) * 100.0,
                    ),
                    fee: total_fee,
                    is_backtest: false,
                    backtest_id: None,
                    notes: None,
                    created_at: position.entry_time.clone(),
                };

                emit_paper_trade(app_handle, &closed_trade);
                notify_strategy_trade(
                    stdin,
                    &position.id,
                    &position.pair,
                    &position.side,
                    close_price,
                    position.quantity,
                    pnl,
                    "close",
                    &runtime,
                );
                push_bot_log(
                    app_handle,
                    "trade",
                    format!(
                        "Closed {} {} at {:.4} (PnL {:+.2})",
                        position.side, position.pair, close_price, pnl
                    ),
                );
            }

            emit_paper_equity(app_handle, &runtime);
        }
        "cancel" => push_bot_log(app_handle, "info", "Cancel request received."),
        _ => push_bot_log(app_handle, "info", line.to_string()),
    }
}

fn add_preflight_check(
    checks: &mut Vec<Value>,
    has_fatal: &mut bool,
    app_handle: &AppHandle,
    id: &str,
    label: &str,
    status: &str,
    message: impl Into<String>,
) {
    let message = message.into();
    if status == "error" {
        *has_fatal = true;
    }
    let log_level = match status {
        "error" => "error",
        "warn" => "warn",
        _ => "info",
    };
    push_bot_log(
        app_handle,
        log_level,
        format!("Preflight {label}: {message}"),
    );
    checks.push(serde_json::json!({
        "id": id,
        "label": label,
        "status": status,
        "message": message,
    }));
}

fn validate_strategy_with_runner(
    python_path: &str,
    strategy_path: &str,
) -> Result<(String, Vec<String>), String> {
    let mut cmd = build_python_command(python_path);
    let output = cmd
        .arg("-m")
        .arg("quantalgo.runner")
        .arg(strategy_path)
        .arg("--validate")
        .output()
        .map_err(|e| format!("Strategy validation failed to start: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let parsed = stdout
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .next();

    if output.status.success() {
        if let Some(ref value) = parsed {
            if value
                .get("ok")
                .and_then(|flag| flag.as_bool())
                .unwrap_or(false)
            {
                let class_name = value
                    .get("class_name")
                    .and_then(|item| item.as_str())
                    .unwrap_or("Strategy");
                let warnings = value
                    .get("warnings")
                    .and_then(|item| item.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|item| item.as_str().map(|value| value.to_string()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                return Ok((format!("Strategy validated: {class_name}"), warnings));
            }
        }
    }

    let runner_error = parsed
        .as_ref()
        .and_then(|value| value.get("error"))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    let detail = runner_error
        .or_else(|| {
            let combined = format!("{}\n{}", stderr.trim(), stdout.trim());
            if combined.trim().is_empty() {
                None
            } else {
                Some(combined)
            }
        })
        .unwrap_or_else(|| format!("Runner exited with status {:?}", output.status.code()));

    Err(detail)
}

fn validate_deploy_config(
    checks: &mut Vec<Value>,
    has_fatal: &mut bool,
    app_handle: &AppHandle,
    config: &Value,
    _trading_mode: &str,
) {
    let timeframe = config
        .get("timeframe")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let timeframe_ok = matches!(timeframe, "1m" | "5m" | "15m" | "1h" | "4h" | "1d" | "1w");
    add_preflight_check(
        checks,
        has_fatal,
        app_handle,
        "timeframe",
        "Timeframe",
        if timeframe_ok { "ok" } else { "error" },
        if timeframe_ok {
            format!("Timeframe: {timeframe}")
        } else {
            "Choose a supported timeframe".to_string()
        },
    );

    let initial_balance = config
        .get("initial_balance")
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    add_preflight_check(
        checks,
        has_fatal,
        app_handle,
        "initial_balance",
        "Initial balance",
        if initial_balance >= 100.0 {
            "ok"
        } else {
            "error"
        },
        if initial_balance >= 100.0 {
            format!("Paper balance: {initial_balance:.2}")
        } else {
            "Paper initial balance must be at least 100".to_string()
        },
    );

    let risk_per_trade = config
        .get("risk_per_trade")
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    add_preflight_check(
        checks,
        has_fatal,
        app_handle,
        "risk_per_trade",
        "Risk per trade",
        if risk_per_trade > 0.0 && risk_per_trade <= MAX_RISK_PER_TRADE_PCT {
            if risk_per_trade > WARN_RISK_PER_TRADE_PCT {
                "warn"
            } else {
                "ok"
            }
        } else {
            "error"
        },
        if risk_per_trade > 0.0 && risk_per_trade <= MAX_RISK_PER_TRADE_PCT {
            format!("Risk per trade: {risk_per_trade:.2}%")
        } else {
            format!("Risk per trade must be greater than 0 and no more than {MAX_RISK_PER_TRADE_PCT:.0}%")
        },
    );

    let max_positions = config
        .get("max_positions")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    add_preflight_check(
        checks,
        has_fatal,
        app_handle,
        "max_positions",
        "Max positions",
        if (1..=MAX_CONCURRENT_POSITIONS).contains(&max_positions) {
            if max_positions > WARN_CONCURRENT_POSITIONS {
                "warn"
            } else {
                "ok"
            }
        } else {
            "error"
        },
        if (1..=MAX_CONCURRENT_POSITIONS).contains(&max_positions) {
            format!("Max open positions: {max_positions}")
        } else {
            format!("Max positions must be between 1 and {MAX_CONCURRENT_POSITIONS}")
        },
    );

    let slippage = config
        .get("slippage")
        .and_then(|value| value.as_f64())
        .unwrap_or(-1.0);
    add_preflight_check(
        checks,
        has_fatal,
        app_handle,
        "slippage",
        "Slippage",
        if slippage >= 0.0 && slippage <= MAX_SLIPPAGE_TOLERANCE_PCT {
            if slippage > WARN_SLIPPAGE_TOLERANCE_PCT {
                "warn"
            } else {
                "ok"
            }
        } else {
            "error"
        },
        if slippage >= 0.0 && slippage <= MAX_SLIPPAGE_TOLERANCE_PCT {
            format!("Slippage tolerance: {slippage:.4}%")
        } else {
            format!("Slippage must be between 0 and {MAX_SLIPPAGE_TOLERANCE_PCT:.0} percent")
        },
    );

    let fee = config
        .get("fee")
        .or_else(|| config.get("paper_fee_pct"))
        .and_then(|value| value.as_f64())
        .unwrap_or(DEFAULT_PAPER_FEE_PCT);
    add_preflight_check(
        checks,
        has_fatal,
        app_handle,
        "paper_fee",
        "Paper fee",
        if fee >= 0.0 && fee <= MAX_PAPER_FEE_PCT {
            if fee > WARN_PAPER_FEE_PCT {
                "warn"
            } else {
                "ok"
            }
        } else {
            "error"
        },
        if fee >= 0.0 && fee <= MAX_PAPER_FEE_PCT {
            format!("Paper fee: {fee:.4}%")
        } else {
            format!("Paper fee must be between 0 and {MAX_PAPER_FEE_PCT:.0} percent")
        },
    );

    add_preflight_check(
        checks,
        has_fatal,
        app_handle,
        "position_model",
        "Position model",
        "ok",
        "Paper runtime enforces one open position per pair and reserves 100% short collateral",
    );
}

fn validate_start_config(config: &Value) -> Result<(), String> {
    let timeframe = config
        .get("timeframe")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if !matches!(timeframe, "1m" | "5m" | "15m" | "1h" | "4h" | "1d" | "1w") {
        return Err("Choose a supported timeframe before starting.".into());
    }

    let initial_balance = config
        .get("initial_balance")
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    if initial_balance < 100.0 {
        return Err("Paper initial balance must be at least 100.".into());
    }

    let risk_per_trade = config
        .get("risk_per_trade")
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    if !(risk_per_trade > 0.0 && risk_per_trade <= MAX_RISK_PER_TRADE_PCT) {
        return Err(format!(
            "Risk per trade must be greater than 0 and no more than {MAX_RISK_PER_TRADE_PCT:.0}%."
        ));
    }

    let max_positions = config
        .get("max_positions")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    if !(1..=MAX_CONCURRENT_POSITIONS).contains(&max_positions) {
        return Err(format!(
            "Max positions must be between 1 and {MAX_CONCURRENT_POSITIONS}."
        ));
    }

    let slippage = config
        .get("slippage")
        .and_then(|value| value.as_f64())
        .unwrap_or(-1.0);
    if !(slippage >= 0.0 && slippage <= MAX_SLIPPAGE_TOLERANCE_PCT) {
        return Err(format!(
            "Slippage must be between 0 and {MAX_SLIPPAGE_TOLERANCE_PCT:.0} percent."
        ));
    }

    let fee = config
        .get("fee")
        .or_else(|| config.get("paper_fee_pct"))
        .and_then(|value| value.as_f64())
        .unwrap_or(DEFAULT_PAPER_FEE_PCT);
    if !(fee >= 0.0 && fee <= MAX_PAPER_FEE_PCT) {
        return Err(format!(
            "Paper fee must be between 0 and {MAX_PAPER_FEE_PCT:.0} percent."
        ));
    }

    Ok(())
}

#[tauri::command]
fn validate_strategy(
    strategy_id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    let mut checks: Vec<Value> = Vec::new();
    let mut has_fatal = false;

    let strategy_row: Option<(String, Option<String>)> = {
        let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
        db.query_row(
            "SELECT file_path, params_json FROM strategies WHERE id = ?1",
            params![strategy_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .ok()
    };

    let Some((strategy_file, strategy_params_json)) = strategy_row else {
        add_preflight_check(
            &mut checks,
            &mut has_fatal,
            &app_handle,
            "strategy_exists",
            "Strategy file",
            "error",
            "Strategy not found in database",
        );
        return Ok(serde_json::json!({ "checks": checks, "can_start": false }));
    };

    if std::path::Path::new(&strategy_file).exists() {
        add_preflight_check(
            &mut checks,
            &mut has_fatal,
            &app_handle,
            "strategy_exists",
            "Strategy file",
            "ok",
            "Strategy file found and readable",
        );
    } else {
        add_preflight_check(
            &mut checks,
            &mut has_fatal,
            &app_handle,
            "strategy_exists",
            "Strategy file",
            "error",
            format!("Strategy file not found: {strategy_file}"),
        );
    }

    let settings = state.settings.lock().map_err(|e| format!("Lock: {e}"))?;
    let python_path = resolve_python_path(&settings);
    drop(settings);

    let python_ok = std::process::Command::new(&python_path)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    add_preflight_check(
        &mut checks,
        &mut has_fatal,
        &app_handle,
        "python_available",
        "Python runtime",
        if python_ok { "ok" } else { "error" },
        if python_ok {
            format!("Python found at '{python_path}'")
        } else {
            format!("Python not found at '{python_path}'. Configure it in Settings.")
        },
    );

    if python_ok && std::path::Path::new(&strategy_file).exists() {
        match validate_strategy_with_runner(&python_path, &strategy_file) {
            Ok((message, warnings)) => {
                add_preflight_check(
                    &mut checks,
                    &mut has_fatal,
                    &app_handle,
                    "strategy_runner_validation",
                    "Strategy runner validation",
                    "ok",
                    message,
                );
                for warning in warnings {
                    add_preflight_check(
                        &mut checks,
                        &mut has_fatal,
                        &app_handle,
                        "strategy_selection",
                        "Strategy selection",
                        "warn",
                        warning,
                    );
                }
            }
            Err(message) => add_preflight_check(
                &mut checks,
                &mut has_fatal,
                &app_handle,
                "strategy_runner_validation",
                "Strategy runner validation",
                "error",
                message,
            ),
        }
    }

    if let Some(params_json) = strategy_params_json.as_deref() {
        match serde_json::from_str::<Value>(params_json) {
            Ok(_) => add_preflight_check(
                &mut checks,
                &mut has_fatal,
                &app_handle,
                "strategy_params",
                "Strategy params",
                "ok",
                "Strategy params JSON is valid",
            ),
            Err(err) => add_preflight_check(
                &mut checks,
                &mut has_fatal,
                &app_handle,
                "strategy_params",
                "Strategy params",
                "error",
                format!("Strategy params JSON is invalid: {err}"),
            ),
        }
    } else {
        add_preflight_check(
            &mut checks,
            &mut has_fatal,
            &app_handle,
            "strategy_params",
            "Strategy params",
            "ok",
            "No custom strategy params",
        );
    }

    Ok(serde_json::json!({
        "checks": checks,
        "can_start": !has_fatal,
    }))
}

#[tauri::command]
fn validate_bot_deploy(
    strategy_id: String,
    exchange_id: String,
    pair: String,
    trading_mode: String,
    config: Option<Value>,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    let mut checks: Vec<Value> = Vec::new();
    let mut has_fatal = false;
    let config = config.unwrap_or_else(|| serde_json::json!({}));

    push_bot_log(
        &app_handle,
        "info",
        format!("Running deploy preflight for strategy {strategy_id} on {pair} ({trading_mode})."),
    );

    // Check 1: Strategy exists and file is readable
    let strategy_row: Option<(String, Option<String>)> = {
        let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
        db.query_row(
            "SELECT file_path, params_json FROM strategies WHERE id = ?1",
            params![strategy_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .ok()
    };
    let strategy_file = strategy_row.as_ref().map(|(path, _)| path.clone());
    let strategy_params_json = strategy_row.as_ref().and_then(|(_, params)| params.clone());

    match strategy_file.as_ref() {
        Some(path) => {
            if std::path::Path::new(path).exists() {
                checks.push(serde_json::json!({
                    "id": "strategy_exists",
                    "label": "Strategy file exists",
                    "status": "ok",
                    "message": "Strategy file found and readable"
                }));
            } else {
                checks.push(serde_json::json!({
                    "id": "strategy_exists",
                    "label": "Strategy file exists",
                    "status": "error",
                    "message": format!("Strategy file not found: {}", path)
                }));
                has_fatal = true;
            }
        }
        None => {
            checks.push(serde_json::json!({
                "id": "strategy_exists",
                "label": "Strategy file exists",
                "status": "error",
                "message": "Strategy not found in database"
            }));
            has_fatal = true;
        }
    }

    // Check 2: Python runtime available
    let settings = state.settings.lock().map_err(|e| format!("Lock: {e}"))?;
    let python_path = resolve_python_path(&settings);
    drop(settings);

    let python_ok = std::process::Command::new(&python_path)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if python_ok {
        checks.push(serde_json::json!({
            "id": "python_available",
            "label": "Python runtime",
            "status": "ok",
            "message": format!("Python found at '{}'", python_path)
        }));
    } else {
        checks.push(serde_json::json!({
            "id": "python_available",
            "label": "Python runtime",
            "status": "error",
            "message": format!("Python not found at '{}'. Configure it in Settings.", python_path)
        }));
        has_fatal = true;
    }

    // Check 3: actual runner validation. This imports, selects, and instantiates the Strategy subclass.
    if python_ok {
        if let Some(ref path) = strategy_file {
            match validate_strategy_with_runner(&python_path, path) {
                Ok((message, warnings)) => {
                    add_preflight_check(
                        &mut checks,
                        &mut has_fatal,
                        &app_handle,
                        "strategy_runner_validation",
                        "Strategy runner validation",
                        "ok",
                        message,
                    );
                    for warning in warnings {
                        add_preflight_check(
                            &mut checks,
                            &mut has_fatal,
                            &app_handle,
                            "strategy_selection",
                            "Strategy selection",
                            "warn",
                            warning,
                        );
                    }
                }
                Err(message) => add_preflight_check(
                    &mut checks,
                    &mut has_fatal,
                    &app_handle,
                    "strategy_runner_validation",
                    "Strategy runner validation",
                    "error",
                    message,
                ),
            }
        }
    }

    if let Some(params_json) = strategy_params_json.as_deref() {
        match serde_json::from_str::<Value>(params_json) {
            Ok(_) => add_preflight_check(
                &mut checks,
                &mut has_fatal,
                &app_handle,
                "strategy_params",
                "Strategy params",
                "ok",
                "Strategy params JSON is valid",
            ),
            Err(err) => add_preflight_check(
                &mut checks,
                &mut has_fatal,
                &app_handle,
                "strategy_params",
                "Strategy params",
                "error",
                format!("Strategy params JSON is invalid: {err}"),
            ),
        }
    } else {
        add_preflight_check(
            &mut checks,
            &mut has_fatal,
            &app_handle,
            "strategy_params",
            "Strategy params",
            "ok",
            "No custom strategy params",
        );
    }

    // Check 4: Exchange configured
    if !exchange_id.is_empty() {
        let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
        let exchange_exists: bool = db
            .query_row(
                "SELECT COUNT(*) FROM exchanges WHERE id = ?1",
                params![exchange_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);

        if exchange_exists {
            checks.push(serde_json::json!({
                "id": "exchange_configured",
                "label": "Exchange configured",
                "status": "ok",
                "message": "Selected exchange found"
            }));
        } else {
            checks.push(serde_json::json!({
                "id": "exchange_configured",
                "label": "Exchange configured",
                "status": "error",
                "message": "Selected exchange not found in database"
            }));
            has_fatal = true;
        }
    } else {
        checks.push(serde_json::json!({
            "id": "exchange_configured",
            "label": "Exchange configured",
            "status": "error",
            "message": "No exchange selected"
        }));
        has_fatal = true;
    }

    let exchange_provider: Option<String> = if exchange_id.is_empty() {
        None
    } else {
        let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
        db.query_row(
            "SELECT provider FROM exchanges WHERE id = ?1",
            params![exchange_id],
            |row| row.get::<_, String>(0),
        )
        .ok()
    };

    // Check 5: Pair selected
    if pair.is_empty() {
        checks.push(serde_json::json!({
            "id": "pair_selected",
            "label": "Trading pair",
            "status": "error",
            "message": "No trading pair selected"
        }));
        has_fatal = true;
    } else {
        checks.push(serde_json::json!({
            "id": "pair_selected",
            "label": "Trading pair",
            "status": "ok",
            "message": format!("Pair: {}", pair)
        }));
    }

    if !pair.is_empty() {
        if let Some(provider) = exchange_provider.as_deref() {
            match fetch_exchange_pairs_for_provider(provider) {
                Ok(pairs) if pairs.iter().any(|available| available == &pair) => {
                    add_preflight_check(
                        &mut checks,
                        &mut has_fatal,
                        &app_handle,
                        "pair_supported",
                        "Trading pair support",
                        "ok",
                        format!("{pair} is listed by {provider}"),
                    );
                }
                Ok(_) => add_preflight_check(
                    &mut checks,
                    &mut has_fatal,
                    &app_handle,
                    "pair_supported",
                    "Trading pair support",
                    "error",
                    format!("{pair} was not found in {provider} market metadata"),
                ),
                Err(err) => add_preflight_check(
                    &mut checks,
                    &mut has_fatal,
                    &app_handle,
                    "pair_supported",
                    "Trading pair support",
                    "error",
                    format!("Could not verify pair support: {err}"),
                ),
            }
        }
    }

    // Check 6: Bot not already running
    {
        let proc = state.bot_process.lock().map_err(|e| format!("Lock: {e}"))?;
        if proc.is_some() {
            checks.push(serde_json::json!({
                "id": "bot_not_running",
                "label": "Bot available",
                "status": "error",
                "message": "Bot is already running. Stop it first."
            }));
            has_fatal = true;
        } else {
            checks.push(serde_json::json!({
                "id": "bot_not_running",
                "label": "Bot available",
                "status": "ok",
                "message": "Bot is available to start"
            }));
        }
    }

    // Check 7: Trading mode
    if trading_mode == "live" {
        checks.push(serde_json::json!({
            "id": "trading_mode",
            "label": "Trading mode",
            "status": "error",
            "message": "Live trading is disabled until real exchange order routing, reconciliation, and safety checks are implemented."
        }));
        has_fatal = true;
    } else if trading_mode == "paper" {
        checks.push(serde_json::json!({
            "id": "trading_mode",
            "label": "Trading mode",
            "status": "ok",
            "message": "Paper trading mode"
        }));
    } else {
        checks.push(serde_json::json!({
            "id": "trading_mode",
            "label": "Trading mode",
            "status": "error",
            "message": format!("Unsupported trading mode: {}", trading_mode)
        }));
        has_fatal = true;
    }

    validate_deploy_config(
        &mut checks,
        &mut has_fatal,
        &app_handle,
        &config,
        &trading_mode,
    );

    let market_timeframe = config
        .get("timeframe")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if trading_mode == "paper"
        && !pair.is_empty()
        && matches!(
            market_timeframe,
            "1m" | "5m" | "15m" | "1h" | "4h" | "1d" | "1w"
        )
    {
        if let Some(provider) = exchange_provider.as_deref() {
            match fetch_latest_market_candle(provider, &pair, market_timeframe) {
                Ok(candle) => add_preflight_check(
                    &mut checks,
                    &mut has_fatal,
                    &app_handle,
                    "market_data",
                    "Public market data",
                    "ok",
                    format!(
                        "{provider} returned {pair} {market_timeframe} close {:.4}",
                        candle.close
                    ),
                ),
                Err(err) => add_preflight_check(
                    &mut checks,
                    &mut has_fatal,
                    &app_handle,
                    "market_data",
                    "Public market data",
                    "error",
                    format!("Could not load public market data: {err}"),
                ),
            }
        }
    }

    push_bot_log(
        &app_handle,
        if has_fatal { "error" } else { "info" },
        if has_fatal {
            "Deploy preflight blocked start; resolve error checks first.".to_string()
        } else {
            "Deploy preflight passed.".to_string()
        },
    );

    Ok(serde_json::json!({
        "checks": checks,
        "can_start": !has_fatal
    }))
}

#[tauri::command]
fn start_bot(
    strategy_id: String,
    exchange_id: String,
    pair: String,
    config: Value,
    trading_mode: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<BotStatus, String> {
    if trading_mode == "live" {
        let message = "Live trading is disabled until real exchange order routing, reconciliation, and safety checks are implemented.".to_string();
        emit_bot_error(&app_handle, message.clone(), None);
        return Err(message);
    }
    if trading_mode != "paper" {
        let message = format!("Unsupported trading mode: {trading_mode}");
        emit_bot_error(&app_handle, message.clone(), None);
        return Err(message);
    }
    if pair.trim().is_empty() {
        return Err("Trading pair is required.".into());
    }
    validate_start_config(&config)?;

    {
        let proc = state.bot_process.lock().map_err(|e| format!("Lock: {e}"))?;
        if proc.is_some() {
            return Err("Bot is already running. Stop it first.".into());
        }
    }

    let (file_path, strategy_params_json) = {
        let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
        db.query_row(
            "SELECT file_path, params_json FROM strategies WHERE id = ?1",
            params![strategy_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .map_err(|e| format!("Strategy not found: {e}"))?
    };

    let exchange_provider = {
        let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
        db.query_row(
            "SELECT provider FROM exchanges WHERE id = ?1",
            params![exchange_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|e| format!("Exchange not found: {e}"))?
    };

    let pairs = fetch_exchange_pairs_for_provider(&exchange_provider)?;
    if !pairs.iter().any(|available| available == &pair) {
        return Err(format!(
            "{pair} was not found in {exchange_provider} market metadata."
        ));
    }

    let settings = state.settings.lock().map_err(|e| format!("Lock: {e}"))?;
    let python_path = resolve_python_path(&settings);
    let default_timeframe = settings.default_timeframe.clone();
    drop(settings);

    let market_timeframe = config
        .get("timeframe")
        .and_then(|value| value.as_str())
        .unwrap_or(default_timeframe.as_str())
        .to_string();
    let mut startup_candles =
        match fetch_recent_market_candles(&exchange_provider, &pair, &market_timeframe, 200) {
            Ok(candles) => candles,
            Err(err) => {
                push_bot_log(
                    &app_handle,
                    "warn",
                    format!("Could not load strategy warm-up candles: {err}"),
                );
                Vec::new()
            }
        };
    let initial_candle = match startup_candles.pop() {
        Some(candle) => candle,
        None => fetch_latest_market_candle(&exchange_provider, &pair, &market_timeframe)?,
    };
    let warmup_candles_json = startup_candles
        .iter()
        .map(|candle| market_candle_json(candle, &pair))
        .collect::<Vec<_>>();

    let mut command = build_python_command(&python_path);
    command
        .arg("-m")
        .arg("quantalgo.runner")
        .arg(&file_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().map_err(|e| {
        let message = format!("Failed to spawn strategy runner: {e}");
        emit_bot_error(&app_handle, message.clone(), None);
        message
    })?;
    let stdin = Arc::new(Mutex::new(
        child
            .stdin
            .take()
            .ok_or_else(|| "Bot stdin unavailable".to_string())?,
    ));
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Bot stdout unavailable".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Bot stderr unavailable".to_string())?;
    let child = Arc::new(Mutex::new(child));
    let stop_flag = Arc::new(AtomicBool::new(false));

    let now = Utc::now().to_rfc3339();
    let config_json_str = serde_json::to_string(&config).ok();
    let initial_balance = config
        .get("initial_balance")
        .and_then(|value| value.as_f64())
        .unwrap_or(10_000.0);

    let fee_rate = config
        .get("fee")
        .or_else(|| config.get("paper_fee_pct"))
        .and_then(|v| v.as_f64())
        .map(|v| v / 100.0)
        .unwrap_or(DEFAULT_PAPER_FEE_PCT / 100.0);
    let slippage_pct = config
        .get("slippage")
        .and_then(|v| v.as_f64())
        .unwrap_or(DEFAULT_SLIPPAGE_TOLERANCE_PCT);
    let risk_per_trade = config
        .get("risk_per_trade")
        .and_then(|v| v.as_f64())
        .unwrap_or(DEFAULT_RISK_PER_TRADE_PCT);
    let max_positions = config
        .get("max_positions")
        .and_then(|v| v.as_u64())
        .unwrap_or(3) as usize;

    let runtime = Arc::new(Mutex::new(LiveBotRuntime {
        strategy_id: strategy_id.clone(),
        exchange_id: exchange_id.clone(),
        pair: pair.clone(),
        balance: initial_balance,
        last_price: initial_candle.close,
        open_positions: Vec::new(),
        fee_rate,
        slippage_pct,
        risk_per_trade,
        max_positions,
    }));

    let strategy_params = strategy_params_json
        .as_deref()
        .map(|value| serde_json::from_str::<Value>(value))
        .transpose()
        .map_err(|err| format!("Strategy params JSON is invalid: {err}"))?
        .unwrap_or_else(|| serde_json::json!({}));

    let (startup_tx, startup_rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let stdout_app = app_handle.clone();
    let stdout_stdin = Arc::clone(&stdin);
    let stdout_runtime = Arc::clone(&runtime);
    let stdout_startup_tx = startup_tx.clone();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line_result in reader.lines() {
            match line_result {
                Ok(line) => {
                    if let Ok(value) = serde_json::from_str::<Value>(&line) {
                        if value.get("id").and_then(|item| item.as_str()) == Some("startup") {
                            let result = if let Some(error) = value.get("error") {
                                Err(error
                                    .get("message")
                                    .and_then(|item| item.as_str())
                                    .unwrap_or("Strategy startup failed")
                                    .to_string())
                            } else {
                                Ok(())
                            };
                            let _ = stdout_startup_tx.send(result);
                            continue;
                        }
                    }
                    handle_strategy_rpc_line(&stdout_app, &stdout_stdin, &stdout_runtime, &line);
                }
                Err(err) => {
                    push_bot_log(&stdout_app, "error", format!("stdout read failed: {err}"));
                    emit_bot_error(
                        &stdout_app,
                        "Bot stdout reader failed.",
                        Some(err.to_string()),
                    );
                    let _ = stdout_startup_tx.send(Err(format!("stdout read failed: {err}")));
                    break;
                }
            }
        }
        let _ = stdout_startup_tx.send(Err(
            "Bot process stdout closed before startup acknowledgement.".into(),
        ));
    });

    let stderr_app = app_handle.clone();
    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line_result in reader.lines() {
            match line_result {
                Ok(line) => {
                    let level = if line.contains("ERROR") {
                        "error"
                    } else if line.contains("WARN") {
                        "warn"
                    } else {
                        "info"
                    };
                    push_bot_log(&stderr_app, level, line.clone());
                }
                Err(err) => {
                    push_bot_log(&stderr_app, "error", format!("stderr read failed: {err}"));
                    emit_bot_error(
                        &stderr_app,
                        "Bot stderr reader failed.",
                        Some(err.to_string()),
                    );
                    break;
                }
            }
        }
    });

    let quote_asset = pair.split('/').nth(1).unwrap_or("USDT").to_string();
    write_json_line(
        &stdin,
        serde_json::json!({
            "id": "startup",
            "method": "start",
            "params": {
                "params": strategy_params,
                "pair": pair.clone(),
                "balance": { quote_asset.clone(): initial_balance },
                "positions": {},
                "mark_price": initial_candle.close,
                "candles": warmup_candles_json
            }
        }),
    )?;

    match startup_rx.recv_timeout(std::time::Duration::from_secs(8)) {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            if let Ok(mut child) = child.lock() {
                let _ = child.kill();
                let _ = child.wait();
            }
            emit_bot_error(&app_handle, "Strategy startup failed.", Some(err.clone()));
            return Err(format!("Strategy startup failed: {err}"));
        }
        Err(_) => {
            if let Ok(mut child) = child.lock() {
                let _ = child.kill();
                let _ = child.wait();
            }
            let message = "Timed out waiting for strategy startup acknowledgement.".to_string();
            emit_bot_error(&app_handle, message.clone(), None);
            return Err(message);
        }
    }

    write_json_line(
        &stdin,
        serde_json::json!({
            "method": "on_candle",
            "params": market_candle_json(&initial_candle, &pair),
        }),
    )?;

    {
        let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
        db.execute(
            "UPDATE bot_state SET status = 'running', strategy_id = ?1, exchange_id = ?2, pair = ?3, started_at = ?4, config_json = ?5, trading_mode = ?6 WHERE id = 'singleton'",
            params![strategy_id, exchange_id, pair, now, config_json_str, trading_mode],
        )
        .map_err(|e| format!("Update bot_state: {e}"))?;
    }

    emit_bot_status(
        &app_handle,
        "running",
        Some(strategy_id.clone()),
        Some(exchange_id.clone()),
        Some(pair.clone()),
        Some(now.clone()),
        &trading_mode,
    );
    push_bot_log(
        &app_handle,
        "info",
        format!(
            "Paper bot started on {} using strategy {}; warmed with {} prior candles.",
            pair,
            strategy_id,
            startup_candles.len(),
        ),
    );
    if let Ok(runtime_guard) = runtime.lock() {
        emit_paper_equity(&app_handle, &runtime_guard);
    }

    let market_app = app_handle.clone();
    let market_stdin = Arc::clone(&stdin);
    let market_runtime = Arc::clone(&runtime);
    let market_stop = Arc::clone(&stop_flag);
    let market_pair = pair.clone();
    let market_provider = exchange_provider.clone();
    let market_child = Arc::clone(&child);
    let market_initial_candle_time = Some(initial_candle.time.clone());
    std::thread::spawn(move || {
        let mut idx = 0_usize;
        let mut last_candle_time: Option<String> = market_initial_candle_time;
        let mut consecutive_errors = 0_usize;
        let poll_seconds = match market_timeframe.as_str() {
            "1m" => 10,
            "5m" | "15m" => 15,
            _ => 30,
        };

        while !market_stop.load(Ordering::Relaxed) {
            match fetch_latest_market_candle(&market_provider, &market_pair, &market_timeframe) {
                Ok(candle) => {
                    consecutive_errors = 0;

                    if let Ok(mut runtime) = market_runtime.lock() {
                        runtime.last_price = candle.close;
                        emit_paper_equity(&market_app, &runtime);
                    }

                    let is_new_candle = last_candle_time.as_deref() != Some(candle.time.as_str());
                    if is_new_candle {
                        let _ = write_json_line(
                            &market_stdin,
                            serde_json::json!({
                                "method": "on_candle",
                                "params": {
                                    "time": candle.time.clone(),
                                    "open": candle.open,
                                    "high": candle.high,
                                    "low": candle.low,
                                    "close": candle.close,
                                    "volume": candle.volume,
                                    "pair": market_pair.clone(),
                                }
                            }),
                        );
                        last_candle_time = Some(candle.time.clone());
                        push_bot_log(
                            &market_app,
                            "info",
                            format!(
                                "Market candle {} {} close {:.4}",
                                market_provider, market_pair, candle.close
                            ),
                        );
                    } else if idx == 0 || idx % 6 == 0 {
                        push_bot_log(
                            &market_app,
                            "info",
                            format!(
                                "Market price {} {} close {:.4}",
                                market_provider, market_pair, candle.close
                            ),
                        );
                    }
                }
                Err(err) => {
                    consecutive_errors += 1;
                    push_bot_log(
                        &market_app,
                        "warn",
                        format!(
                            "Public market data fetch failed for {} {}: {}",
                            market_provider, market_pair, err
                        ),
                    );
                    if consecutive_errors >= 3 {
                        emit_bot_error(
                            &market_app,
                            "Paper market data feed failed.",
                            Some(format!(
                                "Could not fetch {} {} after {} attempts: {}",
                                market_provider, market_pair, consecutive_errors, err
                            )),
                        );
                        if let Ok(mut child) = market_child.lock() {
                            let _ = child.kill();
                        }
                        break;
                    }
                }
            }

            idx += 1;
            std::thread::sleep(std::time::Duration::from_secs(poll_seconds));
        }

        let _ = write_json_line(
            &market_stdin,
            serde_json::json!({ "method": "stop", "params": {} }),
        );
    });

    let watcher_app = app_handle.clone();
    let watcher_child = Arc::clone(&child);
    let watcher_stop = Arc::clone(&stop_flag);
    std::thread::spawn(move || loop {
        let status = {
            let mut child = match watcher_child.lock() {
                Ok(value) => value,
                Err(_) => break,
            };
            child.try_wait().ok().flatten()
        };

        if let Some(exit_status) = status {
            if let Some(state) = watcher_app.try_state::<AppState>() {
                if let Ok(mut proc) = state.bot_process.lock() {
                    *proc = None;
                }
            }

            if watcher_stop.load(Ordering::Relaxed) {
                if let Some(state) = watcher_app.try_state::<AppState>() {
                    if let Ok(db) = state.db.lock() {
                        let _ = db.execute(
                            "UPDATE bot_state SET status = 'stopped', strategy_id = NULL, exchange_id = NULL, pair = NULL, started_at = NULL, config_json = NULL, trading_mode = 'paper' WHERE id = 'singleton'",
                            [],
                        );
                    }
                }
                emit_bot_status(&watcher_app, "stopped", None, None, None, None, "paper");
                push_bot_log(&watcher_app, "info", "Bot process exited after stop.");
            } else {
                let details = format!("Process exit status: {exit_status}");
                emit_bot_error(
                    &watcher_app,
                    "Bot process exited unexpectedly.",
                    Some(details),
                );
                emit_bot_status(&watcher_app, "error", None, None, None, None, "paper");
            }
            break;
        }

        if watcher_stop.load(Ordering::Relaxed) {
            break;
        }

        std::thread::sleep(std::time::Duration::from_millis(300));
    });

    {
        let mut proc = state.bot_process.lock().map_err(|e| format!("Lock: {e}"))?;
        *proc = Some(BotProcess {
            child,
            stdin,
            stop_flag,
        });
    }

    Ok(BotStatus {
        status: "running".into(),
        strategy_id: Some(strategy_id),
        exchange_id: Some(exchange_id),
        pair: Some(pair),
        started_at: Some(now),
        config_json: config_json_str,
        trading_mode,
    })
}

#[tauri::command]
fn stop_bot(app_handle: AppHandle, state: State<'_, AppState>) -> Result<BotStatus, String> {
    let process = {
        let mut proc = state.bot_process.lock().map_err(|e| format!("Lock: {e}"))?;
        proc.take()
    };

    if let Some(bot) = process {
        bot.stop_flag.store(true, Ordering::Relaxed);
        let _ = write_json_line(
            &bot.stdin,
            serde_json::json!({ "method": "stop", "params": {} }),
        );
        if let Ok(mut child) = bot.child.lock() {
            let deadline = Instant::now() + std::time::Duration::from_secs(3);
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if Instant::now() < deadline => {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    _ => {
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                }
            }
        }
    }

    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    db.execute(
        "UPDATE bot_state SET status = 'stopped', strategy_id = NULL, exchange_id = NULL, pair = NULL, started_at = NULL, config_json = NULL, trading_mode = 'paper' WHERE id = 'singleton'",
        [],
    )
    .map_err(|e| format!("Update: {e}"))?;

    emit_bot_status(&app_handle, "stopped", None, None, None, None, "paper");
    push_bot_log(&app_handle, "info", "Bot stopped.");

    Ok(BotStatus {
        status: "stopped".into(),
        strategy_id: None,
        exchange_id: None,
        pair: None,
        started_at: None,
        config_json: None,
        trading_mode: "paper".into(),
    })
}

#[tauri::command]
fn get_bot_status(state: State<'_, AppState>) -> Result<BotStatus, String> {
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    db.query_row(
        "SELECT status, strategy_id, exchange_id, pair, started_at, config_json, trading_mode FROM bot_state WHERE id = 'singleton'",
        [],
        |row| {
            Ok(BotStatus {
                status: row.get(0)?,
                strategy_id: row.get(1)?,
                exchange_id: row.get(2)?,
                pair: row.get(3)?,
                started_at: row.get(4)?,
                config_json: row.get(5)?,
                trading_mode: row.get::<_, Option<String>>(6)?.unwrap_or_else(|| "paper".into()),
            })
        },
    )
    .map_err(|e| format!("Query: {e}"))
}

#[tauri::command]
fn get_bot_logs(
    limit: Option<usize>,
    offset: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<LogEntry>, String> {
    let logs = state.bot_logs.lock().map_err(|e| format!("Lock: {e}"))?;
    let off = offset.unwrap_or(0);
    let lim = limit.unwrap_or(100);
    let slice = if off < logs.len() {
        let end = logs.len().saturating_sub(off);
        let start = end.saturating_sub(lim);
        &logs[start..end]
    } else {
        &[]
    };
    Ok(slice.to_vec())
}

// ---------------------------------------------------------------------------
// Exchange Commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn list_exchanges(state: State<'_, AppState>) -> Result<Vec<Exchange>, String> {
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    let mut stmt = db
        .prepare("SELECT id, name, exchange_type, provider, is_active, created_at, updated_at FROM exchanges ORDER BY name")
        .map_err(|e| format!("Prepare: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Exchange {
                id: row.get(0)?,
                name: row.get(1)?,
                exchange_type: row.get(2)?,
                provider: row.get(3)?,
                is_active: row.get::<_, i32>(4)? != 0,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .map_err(|e| format!("Query: {e}"))?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| format!("Row: {e}"))?);
    }
    Ok(result)
}

#[tauri::command]
fn add_exchange(config: ExchangeConfig, state: State<'_, AppState>) -> Result<Exchange, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let sensitive = serde_json::json!({
        "api_key": config.api_key,
        "api_secret": config.api_secret,
        "passphrase": config.passphrase,
        "wallet_address": config.wallet_address,
        "private_key": config.private_key,
        "rpc_endpoint": config.rpc_endpoint,
    });
    let encrypted = encrypt_string(&sensitive.to_string())?;

    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    db.execute(
        "INSERT INTO exchanges (id, name, exchange_type, provider, config_encrypted, is_active, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7)",
        params![id, config.name, config.exchange_type, config.provider, encrypted, now, now],
    )
    .map_err(|e| format!("Insert: {e}"))?;

    Ok(Exchange {
        id,
        name: config.name,
        exchange_type: config.exchange_type,
        provider: config.provider,
        is_active: true,
        created_at: now.clone(),
        updated_at: now,
    })
}

#[tauri::command]
fn update_exchange(
    id: String,
    config: ExchangeConfig,
    state: State<'_, AppState>,
) -> Result<Exchange, String> {
    let now = Utc::now().to_rfc3339();

    let sensitive = serde_json::json!({
        "api_key": config.api_key,
        "api_secret": config.api_secret,
        "passphrase": config.passphrase,
        "wallet_address": config.wallet_address,
        "private_key": config.private_key,
        "rpc_endpoint": config.rpc_endpoint,
    });
    let encrypted = encrypt_string(&sensitive.to_string())?;

    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    let affected = db
        .execute(
            "UPDATE exchanges SET name = ?1, exchange_type = ?2, provider = ?3, config_encrypted = ?4, updated_at = ?5 WHERE id = ?6",
            params![config.name, config.exchange_type, config.provider, encrypted, now, id],
        )
        .map_err(|e| format!("Update: {e}"))?;

    if affected == 0 {
        return Err("Exchange not found".into());
    }

    let created_at: String = db
        .query_row(
            "SELECT created_at FROM exchanges WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Query: {e}"))?;

    Ok(Exchange {
        id,
        name: config.name,
        exchange_type: config.exchange_type,
        provider: config.provider,
        is_active: true,
        created_at,
        updated_at: now,
    })
}

#[tauri::command]
fn delete_exchange(id: String, state: State<'_, AppState>) -> Result<bool, String> {
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    let affected = db
        .execute("DELETE FROM exchanges WHERE id = ?1", params![id])
        .map_err(|e| format!("Delete: {e}"))?;
    Ok(affected > 0)
}

#[tauri::command]
fn test_exchange_connection(
    id: String,
    state: State<'_, AppState>,
) -> Result<ConnectionResult, String> {
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    let (provider, config_encrypted): (String, Option<String>) = db
        .query_row(
            "SELECT provider, config_encrypted FROM exchanges WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Not found: {e}"))?;
    drop(db);

    // Decrypt credentials
    let creds: Value = match config_encrypted {
        Some(ref enc) => serde_json::from_str(&decrypt_string(enc)?)
            .map_err(|e| format!("Parse credentials: {e}"))?,
        None => {
            return Ok(ConnectionResult {
                success: false,
                message: "No credentials stored for this exchange".into(),
                latency_ms: None,
            })
        }
    };

    let api_key = creds
        .get("api_key")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let api_secret = creds
        .get("api_secret")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let passphrase = creds
        .get("passphrase")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    if api_key.is_empty() || api_secret.is_empty() {
        return Ok(ConnectionResult {
            success: false,
            message: "API key or secret is empty".into(),
            latency_ms: None,
        });
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;

    let start = Instant::now();

    match provider.to_lowercase().as_str() {
        "binance" => {
            // GET /api/v3/account — requires valid API key + HMAC-SHA256 signature
            let ts = get_server_time_ms(&client, "binance");
            let query = format!("timestamp={}", ts);
            let signature = hmac_sha256_hex(api_secret.as_bytes(), query.as_bytes());
            let url = format!("https://api.binance.com/api/v3/account?{}&signature={}", query, signature);

            match client.get(&url).header("X-MBX-APIKEY", api_key).send() {
                Ok(resp) => {
                    let latency = start.elapsed().as_millis() as u64;
                    let status = resp.status();
                    if status.is_success() {
                        Ok(ConnectionResult {
                            success: true,
                            message: format!("Authenticated with Binance ({}ms)", latency),
                            latency_ms: Some(latency),
                        })
                    } else {
                        let body = resp.text().unwrap_or_default();
                        let msg = serde_json::from_str::<Value>(&body)
                            .ok()
                            .and_then(|v| v.get("msg").and_then(|m| m.as_str()).map(String::from))
                            .unwrap_or_else(|| format!("HTTP {}", status));
                        Ok(ConnectionResult {
                            success: false,
                            message: format!("Binance: {}", msg),
                            latency_ms: Some(latency),
                        })
                    }
                }
                Err(e) => Ok(ConnectionResult {
                    success: false,
                    message: format!("Connection failed: {e}"),
                    latency_ms: None,
                }),
            }
        }
        "bybit" => {
            // GET /v5/account/wallet-balance — requires API key + HMAC-SHA256
            let ts = get_server_time_ms(&client, "bybit").to_string();
            let recv_window = "20000";
            let query_string = "accountType=UNIFIED";
            let sign_payload = format!("{}{}{}{}", ts, api_key, recv_window, query_string);
            let signature = hmac_sha256_hex(api_secret.as_bytes(), sign_payload.as_bytes());
            let url = format!("https://api.bybit.com/v5/account/wallet-balance?{}", query_string);

            match client.get(&url)
                .header("X-BAPI-API-KEY", api_key)
                .header("X-BAPI-TIMESTAMP", &ts)
                .header("X-BAPI-SIGN", &signature)
                .header("X-BAPI-RECV-WINDOW", recv_window)
                .send()
            {
                Ok(resp) => {
                    let latency = start.elapsed().as_millis() as u64;
                    let body = resp.text().unwrap_or_default();
                    let json: Value = serde_json::from_str(&body).unwrap_or_default();
                    let ret_code = json.get("retCode").and_then(|v| v.as_i64()).unwrap_or(-1);
                    if ret_code == 0 {
                        Ok(ConnectionResult {
                            success: true,
                            message: format!("Authenticated with Bybit ({}ms)", latency),
                            latency_ms: Some(latency),
                        })
                    } else {
                        let msg = json.get("retMsg").and_then(|v| v.as_str()).unwrap_or("Unknown error");
                        Ok(ConnectionResult {
                            success: false,
                            message: format!("Bybit: {}", msg),
                            latency_ms: Some(latency),
                        })
                    }
                }
                Err(e) => Ok(ConnectionResult {
                    success: false,
                    message: format!("Connection failed: {e}"),
                    latency_ms: None,
                }),
            }
        }
        "okx" => {
            // GET /api/v5/account/balance — requires API key + HMAC-SHA256 (base64) + passphrase
            if passphrase.is_empty() {
                return Ok(ConnectionResult {
                    success: false,
                    message: "OKX requires a passphrase".into(),
                    latency_ms: None,
                });
            }
            let ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
            let method = "GET";
            let path = "/api/v5/account/balance";
            let sign_payload = format!("{}{}{}", ts, method, path);
            let signature = hmac_sha256_base64(api_secret.as_bytes(), sign_payload.as_bytes());
            let url = format!("https://www.okx.com{}", path);

            match client.get(&url)
                .header("OK-ACCESS-KEY", api_key)
                .header("OK-ACCESS-SIGN", &signature)
                .header("OK-ACCESS-TIMESTAMP", &ts)
                .header("OK-ACCESS-PASSPHRASE", passphrase)
                .send()
            {
                Ok(resp) => {
                    let latency = start.elapsed().as_millis() as u64;
                    let body = resp.text().unwrap_or_default();
                    let json: Value = serde_json::from_str(&body).unwrap_or_default();
                    let code = json.get("code").and_then(|v| v.as_str()).unwrap_or("-1");
                    if code == "0" {
                        Ok(ConnectionResult {
                            success: true,
                            message: format!("Authenticated with OKX ({}ms)", latency),
                            latency_ms: Some(latency),
                        })
                    } else {
                        let msg = json.get("msg").and_then(|v| v.as_str()).unwrap_or("Authentication failed");
                        Ok(ConnectionResult {
                            success: false,
                            message: format!("OKX: {}", msg),
                            latency_ms: Some(latency),
                        })
                    }
                }
                Err(e) => Ok(ConnectionResult {
                    success: false,
                    message: format!("Connection failed: {e}"),
                    latency_ms: None,
                }),
            }
        }
        "coinbase" => {
            // GET /api/v3/brokerage/accounts — Coinbase Advanced Trade API
            let ts = chrono::Utc::now().timestamp().to_string();
            let method = "GET";
            let path = "/api/v3/brokerage/accounts";
            let sign_payload = format!("{}{}{}", ts, method, path);
            let signature = hmac_sha256_hex(api_secret.as_bytes(), sign_payload.as_bytes());
            let url = format!("https://api.coinbase.com{}", path);

            match client.get(&url)
                .header("CB-ACCESS-KEY", api_key)
                .header("CB-ACCESS-SIGN", &signature)
                .header("CB-ACCESS-TIMESTAMP", &ts)
                .send()
            {
                Ok(resp) => {
                    let latency = start.elapsed().as_millis() as u64;
                    if resp.status().is_success() {
                        Ok(ConnectionResult {
                            success: true,
                            message: format!("Authenticated with Coinbase ({}ms)", latency),
                            latency_ms: Some(latency),
                        })
                    } else {
                        let body = resp.text().unwrap_or_default();
                        let json: Value = serde_json::from_str(&body).unwrap_or_default();
                        let msg = json.get("message").and_then(|v| v.as_str()).unwrap_or("Authentication failed");
                        Ok(ConnectionResult {
                            success: false,
                            message: format!("Coinbase: {}", msg),
                            latency_ms: Some(latency),
                        })
                    }
                }
                Err(e) => Ok(ConnectionResult {
                    success: false,
                    message: format!("Connection failed: {e}"),
                    latency_ms: None,
                }),
            }
        }
        "kraken" => {
            // POST /0/private/Balance — Kraken uses a different signing scheme (nonce-based, local time is fine)
            let ts = chrono::Utc::now().timestamp_millis().to_string();
            let post_data = format!("nonce={}", ts);
            let path = "/0/private/Balance";

            // Kraken signature: HMAC-SHA512(path + SHA256(nonce + post_data), base64_decode(secret))
            let mut sha = sha2::Sha256::new();
            sha.update(ts.as_bytes());
            sha.update(post_data.as_bytes());
            let sha_hash = sha.finalize();

            let secret_bytes = base64::engine::general_purpose::STANDARD.decode(api_secret)
                .unwrap_or_default();
            let mut path_hash = path.as_bytes().to_vec();
            path_hash.extend_from_slice(&sha_hash);

            type HmacSha512 = hmac::Hmac<sha2::Sha512>;
            let mut mac = <HmacSha512 as Mac>::new_from_slice(&secret_bytes).unwrap_or_else(|_|
                <HmacSha512 as Mac>::new_from_slice(b"invalidkeypadded_to_min_len_xx").unwrap()
            );
            mac.update(&path_hash);
            let signature = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());

            let url = format!("https://api.kraken.com{}", path);

            match client.post(&url)
                .header("API-Key", api_key)
                .header("API-Sign", &signature)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(post_data)
                .send()
            {
                Ok(resp) => {
                    let latency = start.elapsed().as_millis() as u64;
                    let body = resp.text().unwrap_or_default();
                    let json: Value = serde_json::from_str(&body).unwrap_or_default();
                    let errors = json.get("error").and_then(|v| v.as_array());
                    if errors.map(|e| e.is_empty()).unwrap_or(false) {
                        Ok(ConnectionResult {
                            success: true,
                            message: format!("Authenticated with Kraken ({}ms)", latency),
                            latency_ms: Some(latency),
                        })
                    } else {
                        let msg = errors
                            .and_then(|e| e.first())
                            .and_then(|v| v.as_str())
                            .unwrap_or("Authentication failed");
                        Ok(ConnectionResult {
                            success: false,
                            message: format!("Kraken: {}", msg),
                            latency_ms: Some(latency),
                        })
                    }
                }
                Err(e) => Ok(ConnectionResult {
                    success: false,
                    message: format!("Connection failed: {e}"),
                    latency_ms: None,
                }),
            }
        }
        "kucoin" => {
            // GET /api/v1/accounts — requires API key + HMAC-SHA256 (base64) + passphrase
            if passphrase.is_empty() {
                return Ok(ConnectionResult {
                    success: false,
                    message: "KuCoin requires a passphrase".into(),
                    latency_ms: None,
                });
            }
            let ts = get_server_time_ms(&client, "kucoin").to_string();
            let method = "GET";
            let path = "/api/v1/accounts";
            let sign_payload = format!("{}{}{}", ts, method, path);
            let signature = hmac_sha256_base64(api_secret.as_bytes(), sign_payload.as_bytes());
            let passphrase_sign = hmac_sha256_base64(api_secret.as_bytes(), passphrase.as_bytes());
            let url = format!("https://api.kucoin.com{}", path);

            match client.get(&url)
                .header("KC-API-KEY", api_key)
                .header("KC-API-SIGN", &signature)
                .header("KC-API-TIMESTAMP", &ts)
                .header("KC-API-PASSPHRASE", &passphrase_sign)
                .header("KC-API-KEY-VERSION", "2")
                .send()
            {
                Ok(resp) => {
                    let latency = start.elapsed().as_millis() as u64;
                    let body = resp.text().unwrap_or_default();
                    let json: Value = serde_json::from_str(&body).unwrap_or_default();
                    let code = json.get("code").and_then(|v| v.as_str()).unwrap_or("-1");
                    if code == "200000" {
                        Ok(ConnectionResult {
                            success: true,
                            message: format!("Authenticated with KuCoin ({}ms)", latency),
                            latency_ms: Some(latency),
                        })
                    } else {
                        let msg = json.get("msg").and_then(|v| v.as_str()).unwrap_or("Authentication failed");
                        Ok(ConnectionResult {
                            success: false,
                            message: format!("KuCoin: {}", msg),
                            latency_ms: Some(latency),
                        })
                    }
                }
                Err(e) => Ok(ConnectionResult {
                    success: false,
                    message: format!("Connection failed: {e}"),
                    latency_ms: None,
                }),
            }
        }
        other => {
            Ok(ConnectionResult {
                success: false,
                message: format!(
                    "{other} credential validation is not supported. Stored credentials cannot be used for deploy."
                ),
                latency_ms: Some(start.elapsed().as_millis() as u64),
            })
        }
    }
}

#[tauri::command]
fn get_balances(exchange_id: String, state: State<'_, AppState>) -> Result<Vec<Balance>, String> {
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    let (provider, config_encrypted): (String, Option<String>) = db
        .query_row(
            "SELECT provider, config_encrypted FROM exchanges WHERE id = ?1",
            params![exchange_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Exchange not found: {e}"))?;
    drop(db);

    let creds: Value = match config_encrypted {
        Some(ref enc) => serde_json::from_str(&decrypt_string(enc)?)
            .map_err(|e| format!("Parse credentials: {e}"))?,
        None => return Err("No credentials stored".into()),
    };

    let api_key = creds
        .get("api_key")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let api_secret = creds
        .get("api_secret")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    if api_key.is_empty() || api_secret.is_empty() {
        return Err("API key or secret is empty".into());
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;

    match provider.to_lowercase().as_str() {
        "binance" => {
            let ts = get_server_time_ms(&client, "binance");
            let query = format!("timestamp={}", ts);
            let signature = hmac_sha256_hex(api_secret.as_bytes(), query.as_bytes());
            let url = format!(
                "https://api.binance.com/api/v3/account?{}&signature={}",
                query, signature
            );

            let resp = client
                .get(&url)
                .header("X-MBX-APIKEY", api_key)
                .send()
                .map_err(|e| format!("Request failed: {e}"))?;

            if !resp.status().is_success() {
                let body = resp.text().unwrap_or_default();
                let msg = serde_json::from_str::<Value>(&body)
                    .ok()
                    .and_then(|v| v.get("msg").and_then(|m| m.as_str()).map(String::from))
                    .unwrap_or_else(|| "Failed to fetch balances".into());
                return Err(format!("Binance: {}", msg));
            }

            let json: Value = resp.json().map_err(|e| format!("Parse: {e}"))?;
            let balances = json
                .get("balances")
                .and_then(|b| b.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|b| {
                            let asset = b.get("asset").and_then(|v| v.as_str())?;
                            let free: f64 = b.get("free").and_then(|v| v.as_str())?.parse().ok()?;
                            let locked: f64 =
                                b.get("locked").and_then(|v| v.as_str())?.parse().ok()?;
                            let total = free + locked;
                            if total < 0.000001 {
                                return None;
                            }
                            Some(Balance {
                                asset: asset.to_string(),
                                total,
                                available: free,
                                in_positions: locked,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Ok(balances)
        }
        "bybit" => {
            let ts = get_server_time_ms(&client, "bybit").to_string();
            let recv_window = "20000";
            let query_string = "accountType=UNIFIED";
            let sign_payload = format!("{}{}{}{}", ts, api_key, recv_window, query_string);
            let signature = hmac_sha256_hex(api_secret.as_bytes(), sign_payload.as_bytes());
            let url = format!(
                "https://api.bybit.com/v5/account/wallet-balance?{}",
                query_string
            );

            let resp = client
                .get(&url)
                .header("X-BAPI-API-KEY", api_key)
                .header("X-BAPI-TIMESTAMP", &ts)
                .header("X-BAPI-SIGN", &signature)
                .header("X-BAPI-RECV-WINDOW", recv_window)
                .send()
                .map_err(|e| format!("Request failed: {e}"))?;

            let json: Value = resp.json().map_err(|e| format!("Parse: {e}"))?;
            let ret_code = json.get("retCode").and_then(|v| v.as_i64()).unwrap_or(-1);
            if ret_code != 0 {
                let msg = json
                    .get("retMsg")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Failed");
                return Err(format!("Bybit: {}", msg));
            }

            let mut balances = Vec::new();
            if let Some(accounts) = json.pointer("/result/list").and_then(|l| l.as_array()) {
                for account in accounts {
                    if let Some(coins) = account.get("coin").and_then(|c| c.as_array()) {
                        for coin in coins {
                            let asset = coin.get("coin").and_then(|v| v.as_str()).unwrap_or("");
                            let equity: f64 = coin
                                .get("equity")
                                .and_then(|v| v.as_str())
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(0.0);
                            let available: f64 = coin
                                .get("availableToWithdraw")
                                .and_then(|v| v.as_str())
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(0.0);
                            if equity < 0.000001 {
                                continue;
                            }
                            balances.push(Balance {
                                asset: asset.to_string(),
                                total: equity,
                                available,
                                in_positions: (equity - available).max(0.0),
                            });
                        }
                    }
                }
            }
            Ok(balances)
        }
        _ => Err(format!(
            "Balance fetching not yet supported for {}",
            provider
        )),
    }
}

fn fetch_exchange_pairs_for_provider(provider: &str) -> Result<Vec<String>, String> {
    let provider_key = provider.to_lowercase();
    if let Ok(cache) = pair_cache().lock() {
        if let Some(cached) = cache.get(&provider_key) {
            if (Utc::now() - cached.fetched_at).num_seconds() < PAIR_CACHE_TTL_SECS {
                return Ok(cached.pairs.clone());
            }
        }
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;

    let pairs = match provider_key.as_str() {
        "binance" => {
            let resp = client
                .get("https://api.binance.com/api/v3/exchangeInfo")
                .send()
                .map_err(|e| format!("Binance request failed: {e}"))?;
            let json: Value = resp.json().map_err(|e| format!("Parse: {e}"))?;
            json.get("symbols")
                .and_then(|s| s.as_array())
                .map(|syms| {
                    syms.iter()
                        .filter_map(|s| {
                            let status = s.get("status").and_then(|v| v.as_str()).unwrap_or("");
                            if status != "TRADING" {
                                return None;
                            }
                            let base = s.get("baseAsset").and_then(|v| v.as_str())?;
                            let quote = s.get("quoteAsset").and_then(|v| v.as_str())?;
                            Some(format!("{}/{}", base, quote))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        }
        "bybit" => {
            let resp = client
                .get("https://api.bybit.com/v5/market/instruments-info?category=spot&limit=500")
                .send()
                .map_err(|e| format!("Bybit request failed: {e}"))?;
            let json: Value = resp.json().map_err(|e| format!("Parse: {e}"))?;
            json.pointer("/result/list")
                .and_then(|l| l.as_array())
                .map(|list| {
                    list.iter()
                        .filter_map(|item| {
                            let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("");
                            if status != "Trading" {
                                return None;
                            }
                            let base = item.get("baseCoin").and_then(|v| v.as_str())?;
                            let quote = item.get("quoteCoin").and_then(|v| v.as_str())?;
                            Some(format!("{}/{}", base, quote))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        }
        "okx" => {
            let resp = client
                .get("https://www.okx.com/api/v5/public/instruments?instType=SPOT")
                .send()
                .map_err(|e| format!("OKX request failed: {e}"))?;
            let json: Value = resp.json().map_err(|e| format!("Parse: {e}"))?;
            json.get("data")
                .and_then(|d| d.as_array())
                .map(|list| {
                    list.iter()
                        .filter_map(|item| {
                            let state = item.get("state").and_then(|v| v.as_str()).unwrap_or("");
                            if state != "live" {
                                return None;
                            }
                            let base = item.get("baseCcy").and_then(|v| v.as_str())?;
                            let quote = item.get("quoteCcy").and_then(|v| v.as_str())?;
                            Some(format!("{}/{}", base, quote))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        }
        "kraken" => {
            let resp = client
                .get("https://api.kraken.com/0/public/AssetPairs")
                .send()
                .map_err(|e| format!("Kraken request failed: {e}"))?;
            let json: Value = resp.json().map_err(|e| format!("Parse: {e}"))?;
            json.get("result")
                .and_then(|r| r.as_object())
                .map(|pairs_map| {
                    pairs_map
                        .values()
                        .filter_map(|pair| {
                            let base = pair.get("base").and_then(|v| v.as_str())?;
                            let quote = pair.get("quote").and_then(|v| v.as_str())?;
                            // Kraken uses X/Z prefixes for some assets
                            let base_clean = base
                                .strip_prefix('X')
                                .or(base.strip_prefix('Z'))
                                .unwrap_or(base);
                            let quote_clean = quote
                                .strip_prefix('X')
                                .or(quote.strip_prefix('Z'))
                                .unwrap_or(quote);
                            Some(format!("{}/{}", base_clean, quote_clean))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        }
        "kucoin" => {
            let resp = client
                .get("https://api.kucoin.com/api/v1/symbols")
                .send()
                .map_err(|e| format!("KuCoin request failed: {e}"))?;
            let json: Value = resp.json().map_err(|e| format!("Parse: {e}"))?;
            json.get("data")
                .and_then(|d| d.as_array())
                .map(|list| {
                    list.iter()
                        .filter_map(|item| {
                            let enabled = item
                                .get("enableTrading")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            if !enabled {
                                return None;
                            }
                            let base = item.get("baseCurrency").and_then(|v| v.as_str())?;
                            let quote = item.get("quoteCurrency").and_then(|v| v.as_str())?;
                            Some(format!("{}/{}", base, quote))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        }
        "coinbase" => {
            let resp = client
                .get("https://api.exchange.coinbase.com/products")
                .send()
                .map_err(|e| format!("Coinbase request failed: {e}"))?;
            let json: Value = resp.json().map_err(|e| format!("Parse: {e}"))?;
            json.as_array()
                .map(|list| {
                    list.iter()
                        .filter_map(|item| {
                            let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("");
                            if status != "online" {
                                return None;
                            }
                            let base = item.get("base_currency").and_then(|v| v.as_str())?;
                            let quote = item.get("quote_currency").and_then(|v| v.as_str())?;
                            Some(format!("{}/{}", base, quote))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        }
        other => {
            return Err(format!(
                "Public pair discovery is not supported for {other}; configure a supported CEX provider before deploy."
            ));
        }
    };

    if pairs.is_empty() {
        return Err(format!(
            "No active spot pairs returned by {}. Check provider availability before deploy.",
            provider
        ));
    }

    let mut sorted = pairs;
    sorted.sort_by(|a, b| {
        let a_major = a.starts_with("BTC/") || a.starts_with("ETH/") || a.starts_with("SOL/");
        let b_major = b.starts_with("BTC/") || b.starts_with("ETH/") || b.starts_with("SOL/");
        b_major.cmp(&a_major).then(a.cmp(b))
    });
    sorted.dedup();
    if let Ok(mut cache) = pair_cache().lock() {
        cache.insert(
            provider_key,
            CachedPairs {
                fetched_at: Utc::now(),
                pairs: sorted.clone(),
            },
        );
    }
    Ok(sorted)
}

#[tauri::command]
fn get_exchange_pairs(
    exchange_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    let provider: String = db
        .query_row(
            "SELECT provider FROM exchanges WHERE id = ?1",
            params![exchange_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Exchange not found: {e}"))?;
    drop(db);

    fetch_exchange_pairs_for_provider(&provider)
}

// ---------------------------------------------------------------------------
// Trade / Journal Commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn list_trades(filters: TradeFilters, state: State<'_, AppState>) -> Result<Vec<Trade>, String> {
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;

    let mut sql = String::from(
        "SELECT id, strategy_id, exchange, pair, side, entry_price, exit_price, quantity, entry_time, exit_time, pnl, pnl_pct, fee, is_backtest, backtest_id, notes, created_at FROM trades WHERE 1=1",
    );
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(ref v) = filters.strategy_id {
        sql.push_str(&format!(" AND strategy_id = ?{idx}"));
        param_values.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(ref v) = filters.exchange {
        sql.push_str(&format!(" AND exchange = ?{idx}"));
        param_values.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(ref v) = filters.pair {
        sql.push_str(&format!(" AND pair = ?{idx}"));
        param_values.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(ref v) = filters.side {
        sql.push_str(&format!(" AND side = ?{idx}"));
        param_values.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(ref v) = filters.from_date {
        sql.push_str(&format!(" AND entry_time >= ?{idx}"));
        param_values.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(ref v) = filters.to_date {
        sql.push_str(&format!(" AND entry_time <= ?{idx}"));
        param_values.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = filters.min_pnl {
        sql.push_str(&format!(" AND pnl >= ?{idx}"));
        param_values.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = filters.is_backtest {
        let int_val: i32 = if v { 1 } else { 0 };
        sql.push_str(&format!(" AND is_backtest = ?{idx}"));
        param_values.push(Box::new(int_val));
        idx += 1;
    }
    if let Some(ref v) = filters.backtest_id {
        sql.push_str(&format!(" AND backtest_id = ?{idx}"));
        param_values.push(Box::new(v.clone()));
        idx += 1;
    }

    sql.push_str(" ORDER BY entry_time DESC");

    if let Some(limit) = filters.limit {
        sql.push_str(&format!(" LIMIT ?{idx}"));
        param_values.push(Box::new(limit as i64));
        idx += 1;
    }
    if let Some(offset) = filters.offset {
        sql.push_str(&format!(" OFFSET ?{idx}"));
        param_values.push(Box::new(offset as i64));
        let _ = idx; // silence unused assignment warning
    }

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|b| b.as_ref()).collect();

    let mut stmt = db.prepare(&sql).map_err(|e| format!("Prepare: {e}"))?;
    let rows = stmt
        .query_map(params_refs.as_slice(), |row| {
            Ok(Trade {
                id: row.get(0)?,
                strategy_id: row.get(1)?,
                exchange: row.get(2)?,
                pair: row.get(3)?,
                side: row.get(4)?,
                entry_price: row.get(5)?,
                exit_price: row.get(6)?,
                quantity: row.get(7)?,
                entry_time: row.get(8)?,
                exit_time: row.get(9)?,
                pnl: row.get(10)?,
                pnl_pct: row.get(11)?,
                fee: row.get(12)?,
                is_backtest: row.get::<_, i32>(13)? != 0,
                backtest_id: row.get(14)?,
                notes: row.get(15)?,
                created_at: row.get(16)?,
            })
        })
        .map_err(|e| format!("Query: {e}"))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| format!("Row: {e}"))?);
    }
    Ok(result)
}

#[tauri::command]
fn get_trade_stats(
    mut filters: TradeFilters,
    state: State<'_, AppState>,
) -> Result<TradeStats, String> {
    filters.limit = None;
    filters.offset = None;
    let trades = list_trades(filters, state)?;

    if trades.is_empty() {
        return Ok(TradeStats {
            total_trades: 0.0,
            win_rate: 0.0,
            avg_win: 0.0,
            avg_loss: 0.0,
            profit_factor: 0.0,
            expectancy: 0.0,
            best_trade: 0.0,
            worst_trade: 0.0,
            total_pnl: 0.0,
            total_pnl_pct: 0.0,
            avg_duration_secs: 0.0,
        });
    }

    let total = trades.len() as f64;
    let mut wins = 0.0_f64;
    let mut total_win = 0.0_f64;
    let mut total_loss = 0.0_f64;
    let mut win_count = 0_u64;
    let mut loss_count = 0_u64;
    let mut best = f64::MIN;
    let mut worst = f64::MAX;
    let mut total_pnl = 0.0_f64;
    let mut total_pnl_pct = 0.0_f64;
    let mut total_duration = 0.0_f64;
    let mut duration_count = 0_u64;

    for trade in &trades {
        let pnl = trade.pnl.unwrap_or(0.0);
        total_pnl += pnl;
        total_pnl_pct += trade.pnl_pct.unwrap_or(0.0);

        if pnl > best {
            best = pnl;
        }
        if pnl < worst {
            worst = pnl;
        }

        if pnl > 0.0 {
            wins += 1.0;
            total_win += pnl;
            win_count += 1;
        } else if pnl < 0.0 {
            total_loss += pnl.abs();
            loss_count += 1;
        }

        if let (Some(ref exit_time), ref entry_time) = (&trade.exit_time, &trade.entry_time) {
            if let (Ok(entry), Ok(exit)) = (
                chrono::DateTime::parse_from_rfc3339(entry_time),
                chrono::DateTime::parse_from_rfc3339(exit_time),
            ) {
                let dur = (exit - entry).num_seconds().max(0) as f64;
                total_duration += dur;
                duration_count += 1;
            }
        }
    }

    let win_rate = if total > 0.0 { wins / total } else { 0.0 };
    let avg_win = if win_count > 0 {
        total_win / win_count as f64
    } else {
        0.0
    };
    let avg_loss = if loss_count > 0 {
        total_loss / loss_count as f64
    } else {
        0.0
    };
    let profit_factor = if total_loss > 0.0 {
        total_win / total_loss
    } else if total_win > 0.0 {
        f64::INFINITY
    } else {
        0.0
    };
    let expectancy = if total > 0.0 { total_pnl / total } else { 0.0 };
    let avg_duration = if duration_count > 0 {
        total_duration / duration_count as f64
    } else {
        0.0
    };

    if best == f64::MIN {
        best = 0.0;
    }
    if worst == f64::MAX {
        worst = 0.0;
    }

    Ok(TradeStats {
        total_trades: total,
        win_rate,
        avg_win,
        avg_loss,
        profit_factor,
        expectancy,
        best_trade: best,
        worst_trade: worst,
        total_pnl,
        total_pnl_pct,
        avg_duration_secs: avg_duration,
    })
}

#[tauri::command]
fn update_trade_notes(
    id: String,
    notes: String,
    state: State<'_, AppState>,
) -> Result<Trade, String> {
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    let affected = db
        .execute(
            "UPDATE trades SET notes = ?1 WHERE id = ?2",
            params![notes, id],
        )
        .map_err(|e| format!("Update: {e}"))?;

    if affected == 0 {
        return Err("Trade not found".into());
    }

    db.query_row(
        "SELECT id, strategy_id, exchange, pair, side, entry_price, exit_price, quantity, entry_time, exit_time, pnl, pnl_pct, fee, is_backtest, backtest_id, notes, created_at FROM trades WHERE id = ?1",
        params![id],
        |row| {
            Ok(Trade {
                id: row.get(0)?,
                strategy_id: row.get(1)?,
                exchange: row.get(2)?,
                pair: row.get(3)?,
                side: row.get(4)?,
                entry_price: row.get(5)?,
                exit_price: row.get(6)?,
                quantity: row.get(7)?,
                entry_time: row.get(8)?,
                exit_time: row.get(9)?,
                pnl: row.get(10)?,
                pnl_pct: row.get(11)?,
                fee: row.get(12)?,
                is_backtest: row.get::<_, i32>(13)? != 0,
                backtest_id: row.get(14)?,
                notes: row.get(15)?,
                created_at: row.get(16)?,
            })
        },
    )
    .map_err(|e| format!("Query: {e}"))
}

#[tauri::command]
fn get_equity_curve(
    source: Option<String>,
    timeframe: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<EquityPoint>, String> {
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    let src = source.unwrap_or_else(|| "paper".into());

    let mut stmt = db
        .prepare("SELECT timestamp, equity FROM equity_snapshots WHERE source = ?1 ORDER BY timestamp ASC")
        .map_err(|e| format!("Prepare: {e}"))?;

    let rows = stmt
        .query_map(params![src], |row| {
            Ok(EquityPoint {
                time: row.get(0)?,
                equity: row.get(1)?,
            })
        })
        .map_err(|e| format!("Query: {e}"))?;

    let mut points: Vec<EquityPoint> = Vec::new();
    for row in rows {
        points.push(row.map_err(|e| format!("Row: {e}"))?);
    }

    // Apply timeframe downsampling if requested
    if let Some(ref tf) = timeframe {
        let interval_secs: i64 = match tf.as_str() {
            "All" => 0,
            "1m" => 60,
            "5m" => 300,
            "15m" => 900,
            "1h" => 3600,
            "4h" => 14400,
            "1d" => 86400,
            "1w" => 604800,
            _ => 3600,
        };

        if points.len() > 1 && interval_secs > 0 {
            let mut sampled: Vec<EquityPoint> = Vec::new();
            let mut last_bucket: i64 = 0;
            for pt in &points {
                let ts = chrono::DateTime::parse_from_rfc3339(&pt.time)
                    .map(|dt| dt.timestamp())
                    .unwrap_or(0);
                let bucket = ts / interval_secs;
                if bucket != last_bucket || sampled.is_empty() {
                    sampled.push(pt.clone());
                    last_bucket = bucket;
                }
            }
            return Ok(sampled);
        }
    }

    Ok(points)
}

// ---------------------------------------------------------------------------
// Data Commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn export_all_data(state: State<'_, AppState>) -> Result<String, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|e| format!("Lock: {e}"))?
        .clone();
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;

    let mut strategies_stmt = db
        .prepare("SELECT id, name, description, file_path, params_json, created_at, updated_at FROM strategies ORDER BY updated_at DESC")
        .map_err(|e| format!("Prepare strategies: {e}"))?;
    let strategies = strategies_stmt
        .query_map([], |row| {
            Ok(Strategy {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                file_path: row.get(3)?,
                params_json: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .map_err(|e| format!("Query strategies: {e}"))?
        .filter_map(|row| row.ok())
        .map(|strategy| StrategyExport {
            code: std::fs::read_to_string(&strategy.file_path).unwrap_or_default(),
            metadata: strategy,
        })
        .collect::<Vec<_>>();

    let mut backtests_stmt = db
        .prepare("SELECT id, name, strategy_id, config_json, stats_json, equity_curve_json, created_at FROM backtests ORDER BY created_at DESC")
        .map_err(|e| format!("Prepare backtests: {e}"))?;
    let backtests = backtests_stmt
        .query_map([], |row| {
            Ok(BacktestExportRow {
                id: row.get(0)?,
                name: row.get(1)?,
                strategy_id: row.get(2)?,
                config_json: row.get(3)?,
                stats_json: row.get(4)?,
                equity_curve_json: row.get(5)?,
                created_at: row.get(6)?,
            })
        })
        .map_err(|e| format!("Query backtests: {e}"))?
        .filter_map(|row| row.ok())
        .collect::<Vec<_>>();

    let mut exchanges_stmt = db
        .prepare("SELECT id, name, exchange_type, provider, is_active, created_at, updated_at, config_encrypted FROM exchanges ORDER BY name")
        .map_err(|e| format!("Prepare exchanges: {e}"))?;
    let exchanges = exchanges_stmt
        .query_map([], |row| {
            Ok(ExchangeExportRow {
                exchange: Exchange {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    exchange_type: row.get(2)?,
                    provider: row.get(3)?,
                    is_active: row.get::<_, i32>(4)? != 0,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                },
                config_encrypted: row.get(7)?,
            })
        })
        .map_err(|e| format!("Query exchanges: {e}"))?
        .filter_map(|row| row.ok())
        .collect::<Vec<_>>();

    let mut trades_stmt = db
        .prepare("SELECT id, strategy_id, exchange, pair, side, entry_price, exit_price, quantity, entry_time, exit_time, pnl, pnl_pct, fee, is_backtest, backtest_id, notes, created_at FROM trades ORDER BY entry_time DESC")
        .map_err(|e| format!("Prepare trades: {e}"))?;
    let trades = trades_stmt
        .query_map([], |row| {
            Ok(Trade {
                id: row.get(0)?,
                strategy_id: row.get(1)?,
                exchange: row.get(2)?,
                pair: row.get(3)?,
                side: row.get(4)?,
                entry_price: row.get(5)?,
                exit_price: row.get(6)?,
                quantity: row.get(7)?,
                entry_time: row.get(8)?,
                exit_time: row.get(9)?,
                pnl: row.get(10)?,
                pnl_pct: row.get(11)?,
                fee: row.get(12)?,
                is_backtest: row.get::<_, i32>(13)? != 0,
                backtest_id: row.get(14)?,
                notes: row.get(15)?,
                created_at: row.get(16)?,
            })
        })
        .map_err(|e| format!("Query trades: {e}"))?
        .filter_map(|row| row.ok())
        .collect::<Vec<_>>();

    let mut equity_stmt = db
        .prepare("SELECT timestamp, equity, source FROM equity_snapshots ORDER BY timestamp ASC")
        .map_err(|e| format!("Prepare equity snapshots: {e}"))?;
    let equity_snapshots = equity_stmt
        .query_map([], |row| {
            Ok(EquitySnapshotRow {
                timestamp: row.get(0)?,
                equity: row.get(1)?,
                source: row.get(2)?,
            })
        })
        .map_err(|e| format!("Query equity snapshots: {e}"))?
        .filter_map(|row| row.ok())
        .collect::<Vec<_>>();

    let bot_state = db
        .query_row(
            "SELECT status, strategy_id, exchange_id, pair, started_at, config_json, trading_mode FROM bot_state WHERE id = 'singleton'",
            [],
            |row| {
                Ok(BotStatus {
                    status: row.get(0)?,
                    strategy_id: row.get(1)?,
                    exchange_id: row.get(2)?,
                    pair: row.get(3)?,
                    started_at: row.get(4)?,
                    config_json: row.get(5)?,
                    trading_mode: row.get::<_, Option<String>>(6)?.unwrap_or_else(|| "paper".into()),
                })
            },
        )
        .map_err(|e| format!("Query bot state: {e}"))?;

    let export = DataExport {
        exported_at: Utc::now().to_rfc3339(),
        settings,
        strategies,
        backtests,
        exchanges,
        trades,
        equity_snapshots,
        bot_state,
    };

    let export_dir = get_data_dir().join("exports");
    std::fs::create_dir_all(&export_dir).map_err(|e| format!("Create export dir: {e}"))?;
    let path = export_dir.join(format!(
        "quantalgo-export-{}.json",
        Utc::now().format("%Y%m%d-%H%M%S")
    ));

    let json =
        serde_json::to_string_pretty(&export).map_err(|e| format!("Serialize export: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Write export: {e}"))?;

    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
fn import_data(state: State<'_, AppState>) -> Result<String, String> {
    let path = latest_export_file()?;
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("Read import: {e}"))?;
    let mut export: DataExport =
        serde_json::from_str(&raw).map_err(|e| format!("Parse import: {e}"))?;

    normalize_settings_units(&mut export.settings);
    validate_app_settings(&export.settings)?;
    save_settings_to_disk(&export.settings)?;
    {
        let mut settings = state.settings.lock().map_err(|e| format!("Lock: {e}"))?;
        *settings = export.settings.clone();
    }

    let strategy_dir = PathBuf::from(&export.settings.strategy_dir);
    std::fs::create_dir_all(&strategy_dir).map_err(|e| format!("Create strategy dir: {e}"))?;

    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    db.execute_batch(
        "
        DELETE FROM trades;
        DELETE FROM backtests;
        DELETE FROM exchanges;
        DELETE FROM strategies;
        DELETE FROM equity_snapshots;
        DELETE FROM bot_state;
        INSERT INTO bot_state (id, status, strategy_id, exchange_id, pair, started_at, config_json, trading_mode)
        VALUES ('singleton', 'stopped', NULL, NULL, NULL, NULL, NULL, 'paper');
        ",
    )
    .map_err(|e| format!("Reset database: {e}"))?;

    for strategy in export.strategies {
        let file_name = PathBuf::from(&strategy.metadata.file_path)
            .file_name()
            .and_then(|value| value.to_str())
            .map(|value| value.to_string())
            .unwrap_or_else(|| format!("strategy_{}.py", strategy.metadata.id));
        let target_path = strategy_dir.join(file_name);
        std::fs::write(&target_path, strategy.code)
            .map_err(|e| format!("Write strategy file: {e}"))?;

        db.execute(
            "INSERT INTO strategies (id, name, description, file_path, params_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                strategy.metadata.id,
                strategy.metadata.name,
                strategy.metadata.description,
                target_path.to_string_lossy().to_string(),
                strategy.metadata.params_json,
                strategy.metadata.created_at,
                strategy.metadata.updated_at,
            ],
        )
        .map_err(|e| format!("Insert strategy: {e}"))?;
    }

    for backtest in export.backtests {
        db.execute(
            "INSERT INTO backtests (id, name, strategy_id, config_json, stats_json, equity_curve_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                backtest.id,
                backtest.name,
                backtest.strategy_id,
                backtest.config_json,
                backtest.stats_json,
                backtest.equity_curve_json,
                backtest.created_at,
            ],
        )
        .map_err(|e| format!("Insert backtest: {e}"))?;
    }

    for exchange in export.exchanges {
        db.execute(
            "INSERT INTO exchanges (id, name, exchange_type, provider, config_encrypted, is_active, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                exchange.exchange.id,
                exchange.exchange.name,
                exchange.exchange.exchange_type,
                exchange.exchange.provider,
                exchange.config_encrypted,
                if exchange.exchange.is_active { 1 } else { 0 },
                exchange.exchange.created_at,
                exchange.exchange.updated_at,
            ],
        )
        .map_err(|e| format!("Insert exchange: {e}"))?;
    }

    for trade in export.trades {
        db.execute(
            "INSERT INTO trades (id, strategy_id, exchange, pair, side, entry_price, exit_price, quantity, entry_time, exit_time, pnl, pnl_pct, fee, is_backtest, backtest_id, notes, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
            params![
                trade.id,
                trade.strategy_id,
                trade.exchange,
                trade.pair,
                trade.side,
                trade.entry_price,
                trade.exit_price,
                trade.quantity,
                trade.entry_time,
                trade.exit_time,
                trade.pnl,
                trade.pnl_pct,
                trade.fee,
                if trade.is_backtest { 1 } else { 0 },
                trade.backtest_id,
                trade.notes,
                trade.created_at,
            ],
        )
        .map_err(|e| format!("Insert trade: {e}"))?;
    }

    for snapshot in export.equity_snapshots {
        db.execute(
            "INSERT INTO equity_snapshots (timestamp, equity, source) VALUES (?1, ?2, ?3)",
            params![snapshot.timestamp, snapshot.equity, snapshot.source],
        )
        .map_err(|e| format!("Insert snapshot: {e}"))?;
    }

    db.execute(
        "UPDATE bot_state SET status = ?1, strategy_id = ?2, exchange_id = ?3, pair = ?4, started_at = ?5, config_json = ?6, trading_mode = ?7 WHERE id = 'singleton'",
        params![
            export.bot_state.status,
            export.bot_state.strategy_id,
            export.bot_state.exchange_id,
            export.bot_state.pair,
            export.bot_state.started_at,
            export.bot_state.config_json,
            export.bot_state.trading_mode,
        ],
    )
    .map_err(|e| format!("Restore bot state: {e}"))?;

    Ok(path.to_string_lossy().to_string())
}

// ---------------------------------------------------------------------------
// Settings Commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let settings = state.settings.lock().map_err(|e| format!("Lock: {e}"))?;
    Ok(settings.clone())
}

#[tauri::command]
fn update_settings(
    mut settings: AppSettings,
    state: State<'_, AppState>,
) -> Result<AppSettings, String> {
    normalize_settings_units(&mut settings);
    validate_app_settings(&settings)?;
    save_settings_to_disk(&settings)?;
    let mut current = state.settings.lock().map_err(|e| format!("Lock: {e}"))?;
    *current = settings.clone();
    Ok(settings)
}

#[tauri::command]
fn detect_python() -> Result<Option<String>, String> {
    let candidates = if cfg!(windows) {
        vec!["python3", "python", "py"]
    } else {
        vec!["python3", "python"]
    };

    for candidate in candidates {
        let output = std::process::Command::new(candidate)
            .arg("--version")
            .output();
        if let Ok(out) = output {
            if out.status.success() {
                let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let version = if version.is_empty() {
                    String::from_utf8_lossy(&out.stderr).trim().to_string()
                } else {
                    version
                };
                // Verify it's Python 3
                if version.contains("Python 3") {
                    return Ok(Some(candidate.to_string()));
                }
            }
        }
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// Run
// ---------------------------------------------------------------------------

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let data_dir = get_data_dir();
            std::fs::create_dir_all(&data_dir)?;
            std::fs::create_dir_all(data_dir.join("strategies"))?;
            std::fs::create_dir_all(data_dir.join("backtests"))?;
            std::fs::create_dir_all(data_dir.join("logs"))?;

            let db = Connection::open(data_dir.join("quantalgo.db"))?;
            db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
            init_db(&db)?;

            let strategy_dir = data_dir.join("strategies");
            if let Err(e) = seed_strategies(&db, &strategy_dir) {
                eprintln!("[quantalgo] seed_strategies: {e}");
            }
            if let Err(e) = run_migrations(&db, &strategy_dir) {
                eprintln!("[quantalgo] run_migrations: {e}");
            }

            let settings = load_settings_from_disk();

            let state = AppState {
                db: Mutex::new(db),
                bot_process: Mutex::new(None),
                settings: Mutex::new(settings),
                bot_logs: Mutex::new(load_persisted_bot_logs(1_000)),
            };

            app.manage(state);

            // Reconcile stale bot state — if DB says "running" but no process exists, reset to stopped
            {
                let state = app.state::<AppState>();
                let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
                let current_status: String = db
                    .query_row("SELECT status FROM bot_state WHERE id = 'singleton'", [], |row| row.get(0))
                    .unwrap_or_else(|_| "stopped".to_string());
                if current_status == "running" {
                    let _ = db.execute(
                        "UPDATE bot_state SET status = 'stopped', strategy_id = NULL, exchange_id = NULL, pair = NULL, started_at = NULL, config_json = NULL, trading_mode = 'paper' WHERE id = 'singleton'",
                        [],
                    );
                    push_bot_log(
                        app.handle(),
                        "warn",
                        "Previous bot session was marked running but no process was attached; reset to stopped.",
                    );
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_strategies,
            get_strategy,
            create_strategy,
            save_strategy,
            update_strategy_meta,
            delete_strategy,
            read_strategy_file,
            run_backtest,
            list_backtests,
            get_backtest,
            save_backtest,
            delete_backtest,
            validate_strategy,
            validate_bot_deploy,
            start_bot,
            stop_bot,
            get_bot_status,
            get_bot_logs,
            list_exchanges,
            add_exchange,
            update_exchange,
            delete_exchange,
            test_exchange_connection,
            get_balances,
            get_exchange_pairs,
            list_trades,
            get_trade_stats,
            update_trade_notes,
            get_equity_curve,
            export_all_data,
            import_data,
            get_settings,
            update_settings,
            detect_python,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
