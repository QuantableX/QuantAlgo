use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use chrono::Utc;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::Mutex;
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
    pub killer: Box<dyn portable_pty::MasterPty + Send>,
    pub child: Box<dyn portable_pty::Child + Send>,
}

pub struct AppState {
    pub db: Mutex<Connection>,
    pub bot_process: Mutex<Option<BotProcess>>,
    pub settings: Mutex<AppSettings>,
    pub bot_logs: Mutex<Vec<LogEntry>>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
        python_path: "python3".into(),
        strategy_dir: data_dir.join("strategies").to_string_lossy().into_owned(),
        backtest_dir: data_dir.join("backtests").to_string_lossy().into_owned(),
        risk_per_trade: 0.02,
        max_concurrent_positions: 3,
        slippage_tolerance: 0.001,
        notify_on_trade: true,
        notify_on_error: true,
        notify_on_daily_summary: false,
    }
}

const APP_ENCRYPTION_KEY: &[u8; 32] = b"QuantAlgo_AES256_Key_2024!@#$%^&";

pub fn encrypt_string(plaintext: &str) -> Result<String, String> {
    let cipher =
        Aes256Gcm::new_from_slice(APP_ENCRYPTION_KEY).map_err(|e| format!("Cipher init: {e}"))?;
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

pub fn decrypt_string(hex_str: &str) -> Result<String, String> {
    let data = hex::decode(hex_str).map_err(|e| format!("Hex decode: {e}"))?;
    if data.len() < 13 {
        return Err("Ciphertext too short".into());
    }
    let (nonce_bytes, ciphertext) = data.split_at(12);
    let cipher =
        Aes256Gcm::new_from_slice(APP_ENCRYPTION_KEY).map_err(|e| format!("Cipher init: {e}"))?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decrypt: {e}"))?;
    String::from_utf8(plaintext).map_err(|e| format!("UTF-8: {e}"))
}

fn load_settings_from_disk() -> AppSettings {
    let path = get_data_dir().join("config.json");
    if let Ok(data) = std::fs::read_to_string(&path) {
        serde_json::from_str(&data).unwrap_or_else(|_| get_default_settings())
    } else {
        get_default_settings()
    }
}

fn save_settings_to_disk(settings: &AppSettings) -> Result<(), String> {
    let path = get_data_dir().join("config.json");
    let json = serde_json::to_string_pretty(settings).map_err(|e| format!("Serialize: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Write config: {e}"))
}

const STRATEGY_TEMPLATE: &str = r#"from quantalgo import Strategy


class NewStrategy(Strategy):
    """Strategy description here."""

    params = {
        "fast_period": 12,
        "slow_period": 26,
        "risk_per_trade": 0.02,
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
            config_json TEXT
        );

        CREATE TABLE IF NOT EXISTS equity_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            equity REAL NOT NULL,
            source TEXT DEFAULT 'live'
        );

        INSERT OR IGNORE INTO bot_state (id, status) VALUES ('singleton', 'stopped');
        ",
    )?;
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
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
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
) -> Result<BacktestResult, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let _ = app_handle.emit("backtest:started", serde_json::json!({ "id": &id }));

    let strategy_id = config.strategy_id.clone();

    let settings_path = get_data_dir().join("config.json");
    let python_path = if let Ok(data) = std::fs::read_to_string(&settings_path) {
        let val: Value = serde_json::from_str(&data).unwrap_or(Value::Null);
        val.get("python_path")
            .and_then(|v| v.as_str())
            .unwrap_or("python3")
            .to_string()
    } else {
        "python3".into()
    };

    let config_json =
        serde_json::to_string(&config).map_err(|e| format!("Serialize config: {e}"))?;

    let pty_system = native_pty_system();
    let pty_pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("PTY open: {e}"))?;

    let mut cmd = CommandBuilder::new(&python_path);
    cmd.arg("-m");
    cmd.arg("quantalgo");
    cmd.arg("--backtest");
    cmd.arg("--config");
    cmd.arg(&config_json);

    let mut child = pty_pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Spawn: {e}"))?;

    let reader = pty_pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("Clone reader: {e}"))?;
    let buf_reader = BufReader::new(reader);

    let mut output_lines: Vec<String> = Vec::new();
    let handle_clone = app_handle.clone();
    let bt_id = id.clone();

    for line_result in buf_reader.lines() {
        match line_result {
            Ok(line) => {
                let _ = handle_clone.emit(
                    "backtest:log",
                    serde_json::json!({ "id": &bt_id, "line": &line }),
                );
                output_lines.push(line);
            }
            Err(_) => break,
        }
    }

    let exit = child.wait().map_err(|e| format!("Wait: {e}"))?;
    drop(pty_pair.master);

    let full_output = output_lines.join("\n");

    let (stats, equity_curve, trades) =
        parse_backtest_output(&full_output, &config).unwrap_or_else(|_| {
            (
                BacktestStats {
                    total_return: 0.0,
                    total_return_pct: 0.0,
                    sharpe_ratio: 0.0,
                    max_drawdown: 0.0,
                    max_drawdown_pct: 0.0,
                    win_rate: 0.0,
                    profit_factor: 0.0,
                    total_trades: 0,
                    avg_trade_duration_secs: 0.0,
                },
                vec![EquityPoint {
                    time: config.start_date.clone(),
                    equity: config.initial_capital,
                }],
                Vec::new(),
            )
        });

    let result = BacktestResult {
        id: id.clone(),
        name: format!(
            "Backtest {} {}",
            config.pair,
            Utc::now().format("%Y-%m-%d %H:%M")
        ),
        strategy_id,
        config,
        stats,
        equity_curve,
        trades,
        created_at: now,
    };

    let status = if exit.success() {
        "completed"
    } else {
        "failed"
    };
    let _ = app_handle.emit(
        "backtest:finished",
        serde_json::json!({ "id": &id, "status": status }),
    );

    Ok(result)
}

fn parse_backtest_output(
    output: &str,
    config: &BacktestConfig,
) -> Result<(BacktestStats, Vec<EquityPoint>, Vec<Trade>), String> {
    // Try to parse the last line as a JSON result from the Python backtest runner.
    // Expected format: {"stats": {...}, "equity_curve": [...], "trades": [...]}
    for line in output.lines().rev() {
        let trimmed = line.trim();
        if trimmed.starts_with('{') {
            if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
                let stats: BacktestStats = serde_json::from_value(
                    val.get("stats").cloned().unwrap_or(Value::Null),
                )
                .unwrap_or(BacktestStats {
                    total_return: 0.0,
                    total_return_pct: 0.0,
                    sharpe_ratio: 0.0,
                    max_drawdown: 0.0,
                    max_drawdown_pct: 0.0,
                    win_rate: 0.0,
                    profit_factor: 0.0,
                    total_trades: 0,
                    avg_trade_duration_secs: 0.0,
                });

                let equity_curve: Vec<EquityPoint> = serde_json::from_value(
                    val.get("equity_curve").cloned().unwrap_or(Value::Array(vec![])),
                )
                .unwrap_or_default();

                let trades: Vec<Trade> = serde_json::from_value(
                    val.get("trades").cloned().unwrap_or(Value::Array(vec![])),
                )
                .unwrap_or_default();

                return Ok((stats, equity_curve, trades));
            }
        }
    }

    // Fallback: return empty stats with initial capital equity point
    Ok((
        BacktestStats {
            total_return: 0.0,
            total_return_pct: 0.0,
            sharpe_ratio: 0.0,
            max_drawdown: 0.0,
            max_drawdown_pct: 0.0,
            win_rate: 0.0,
            profit_factor: 0.0,
            total_trades: 0,
            avg_trade_duration_secs: 0.0,
        },
        vec![EquityPoint {
            time: config.start_date.clone(),
            equity: config.initial_capital,
        }],
        Vec::new(),
    ))
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
    let eq_data: EqData =
        serde_json::from_str(&eq_json).unwrap_or(EqData {
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
    let stats_json =
        serde_json::to_string(&result.stats).map_err(|e| format!("Serialize: {e}"))?;
    let eq_json = serde_json::to_string(&serde_json::json!({
        "equity_curve": result.equity_curve,
        "trades": result.trades,
    }))
    .map_err(|e| format!("Serialize: {e}"))?;

    db.execute(
        "INSERT INTO backtests (id, name, strategy_id, config_json, stats_json, equity_curve_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
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
    db.execute(
        "DELETE FROM trades WHERE backtest_id = ?1",
        params![id],
    )
    .map_err(|e| format!("Delete trades: {e}"))?;
    let affected = db
        .execute("DELETE FROM backtests WHERE id = ?1", params![id])
        .map_err(|e| format!("Delete: {e}"))?;
    Ok(affected > 0)
}

// ---------------------------------------------------------------------------
// Bot Commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn start_bot(
    strategy_id: String,
    exchange_id: String,
    pair: String,
    config: Value,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<BotStatus, String> {
    // Check if already running
    {
        let proc = state.bot_process.lock().map_err(|e| format!("Lock: {e}"))?;
        if proc.is_some() {
            return Err("Bot is already running. Stop it first.".into());
        }
    }

    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    let file_path: String = db
        .query_row(
            "SELECT file_path FROM strategies WHERE id = ?1",
            params![strategy_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Strategy not found: {e}"))?;
    drop(db);

    let settings = state.settings.lock().map_err(|e| format!("Lock: {e}"))?;
    let python_path = settings.python_path.clone();
    drop(settings);

    let pty_system = native_pty_system();
    let pty_pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("PTY open: {e}"))?;

    let mut cmd = CommandBuilder::new(&python_path);
    cmd.arg("-u"); // unbuffered output
    cmd.arg(&file_path);
    cmd.arg("--exchange");
    cmd.arg(&exchange_id);
    cmd.arg("--pair");
    cmd.arg(&pair);
    if config != Value::Null {
        cmd.arg("--config");
        cmd.arg(serde_json::to_string(&config).unwrap_or_default());
    }

    let child = pty_pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Spawn: {e}"))?;

    let reader = pty_pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("Clone reader: {e}"))?;

    // Clear previous logs
    {
        let mut logs = state.bot_logs.lock().map_err(|e| format!("Lock: {e}"))?;
        logs.clear();
    }

    let now = Utc::now().to_rfc3339();
    let config_json_str = serde_json::to_string(&config).ok();

    // Update bot_state in DB
    {
        let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
        db.execute(
            "UPDATE bot_state SET status = 'running', strategy_id = ?1, exchange_id = ?2, pair = ?3, started_at = ?4, config_json = ?5 WHERE id = 'singleton'",
            params![strategy_id, exchange_id, pair, now, config_json_str],
        )
        .map_err(|e| format!("Update bot_state: {e}"))?;
    }

    // Spawn a thread to read PTY output and emit events
    let handle_clone = app_handle.clone();
    // We need a way to append to bot_logs from the background thread.
    // Since AppState is managed by Tauri, we'll use the app_handle to access it.
    let app_for_logs = app_handle.clone();
    std::thread::spawn(move || {
        let buf_reader = BufReader::new(reader);
        for line_result in buf_reader.lines() {
            match line_result {
                Ok(line) => {
                    let entry = LogEntry {
                        timestamp: Utc::now().to_rfc3339(),
                        level: if line.contains("ERROR") || line.contains("error") {
                            "error".into()
                        } else if line.contains("WARN") || line.contains("warn") {
                            "warn".into()
                        } else {
                            "info".into()
                        },
                        message: line.clone(),
                    };
                    let _ = handle_clone.emit("bot:log", &entry);
                    if let Some(state) = app_for_logs.try_state::<AppState>() {
                        if let Ok(mut logs) = state.bot_logs.lock() {
                            logs.push(entry);
                            // Keep at most 10000 log entries in memory
                            if logs.len() > 10000 {
                                logs.drain(0..1000);
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }
        // Process ended; update state
        if let Some(state) = app_for_logs.try_state::<AppState>() {
            if let Ok(mut proc) = state.bot_process.lock() {
                *proc = None;
            }
            if let Ok(db) = state.db.lock() {
                let _ = db.execute(
                    "UPDATE bot_state SET status = 'stopped', started_at = NULL WHERE id = 'singleton'",
                    [],
                );
            }
        }
        let _ = handle_clone.emit("bot:stopped", serde_json::json!({}));
    });

    // Store the process handle
    {
        let mut proc = state.bot_process.lock().map_err(|e| format!("Lock: {e}"))?;
        *proc = Some(BotProcess {
            killer: pty_pair.master,
            child,
        });
    }

    let _ = app_handle.emit("bot:started", serde_json::json!({ "strategy_id": &strategy_id, "pair": &pair }));

    Ok(BotStatus {
        status: "running".into(),
        strategy_id: Some(strategy_id),
        exchange_id: Some(exchange_id),
        pair: Some(pair),
        started_at: Some(now),
        config_json: config_json_str,
    })
}

#[tauri::command]
fn stop_bot(state: State<'_, AppState>) -> Result<BotStatus, String> {
    let mut proc = state.bot_process.lock().map_err(|e| format!("Lock: {e}"))?;
    if let Some(mut bot) = proc.take() {
        // Send Ctrl+C (ETX byte) through the PTY master to gracefully signal the child
        {
            let mut writer = bot.killer.take_writer().map_err(|e| format!("PTY writer: {e}"))?;
            let _ = std::io::Write::write_all(&mut writer, b"\x03");
        }
        // Give the process a moment, then kill if still alive
        std::thread::sleep(std::time::Duration::from_millis(500));
        let _ = bot.child.kill();
        let _ = bot.child.wait();
    }

    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    db.execute(
        "UPDATE bot_state SET status = 'stopped', started_at = NULL WHERE id = 'singleton'",
        [],
    )
    .map_err(|e| format!("Update: {e}"))?;

    Ok(BotStatus {
        status: "stopped".into(),
        strategy_id: None,
        exchange_id: None,
        pair: None,
        started_at: None,
        config_json: None,
    })
}

#[tauri::command]
fn get_bot_status(state: State<'_, AppState>) -> Result<BotStatus, String> {
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    db.query_row(
        "SELECT status, strategy_id, exchange_id, pair, started_at, config_json FROM bot_state WHERE id = 'singleton'",
        [],
        |row| {
            Ok(BotStatus {
                status: row.get(0)?,
                strategy_id: row.get(1)?,
                exchange_id: row.get(2)?,
                pair: row.get(3)?,
                started_at: row.get(4)?,
                config_json: row.get(5)?,
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
        let end = (off + lim).min(logs.len());
        &logs[off..end]
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
fn test_exchange_connection(id: String, state: State<'_, AppState>) -> Result<ConnectionResult, String> {
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    let (provider, _config_encrypted): (String, Option<String>) = db
        .query_row(
            "SELECT provider, config_encrypted FROM exchanges WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Not found: {e}"))?;
    drop(db);

    let url = match provider.to_lowercase().as_str() {
        "binance" => "https://api.binance.com/api/v3/ping",
        "bybit" => "https://api.bybit.com/v5/market/time",
        "okx" => "https://www.okx.com/api/v5/public/time",
        "coinbase" => "https://api.coinbase.com/v2/time",
        "kraken" => "https://api.kraken.com/0/public/Time",
        "kucoin" => "https://api.kucoin.com/api/v1/timestamp",
        "bitget" => "https://api.bitget.com/api/v2/public/time",
        "gate" | "gateio" => "https://api.gateio.ws/api/v4/spot/time",
        _ => "https://httpbin.org/get",
    };

    let start = Instant::now();
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;

    let response: Result<reqwest::blocking::Response, reqwest::Error> = client.get(url).send();
    match response {
        Ok(resp) => {
            let latency = start.elapsed().as_millis() as u64;
            if resp.status().is_success() {
                Ok(ConnectionResult {
                    success: true,
                    message: format!("Connected to {} ({}ms)", provider, latency),
                    latency_ms: Some(latency),
                })
            } else {
                Ok(ConnectionResult {
                    success: false,
                    message: format!("HTTP {}: {}", resp.status(), provider),
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

#[tauri::command]
fn get_balances(exchange_id: String, state: State<'_, AppState>) -> Result<Vec<Balance>, String> {
    // Verify exchange exists
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    let _name: String = db
        .query_row(
            "SELECT name FROM exchanges WHERE id = ?1",
            params![exchange_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Exchange not found: {e}"))?;
    drop(db);

    // Return placeholder balances. Real exchange integration would use the
    // decrypted API keys to call the exchange balance endpoint.
    Ok(vec![
        Balance {
            asset: "USDT".into(),
            total: 10000.0,
            available: 8500.0,
            in_positions: 1500.0,
        },
        Balance {
            asset: "BTC".into(),
            total: 0.5,
            available: 0.3,
            in_positions: 0.2,
        },
        Balance {
            asset: "ETH".into(),
            total: 5.0,
            available: 5.0,
            in_positions: 0.0,
        },
    ])
}

#[tauri::command]
fn get_exchange_pairs(exchange_id: String, state: State<'_, AppState>) -> Result<Vec<String>, String> {
    // Verify exchange exists
    let db = state.db.lock().map_err(|e| format!("Lock: {e}"))?;
    let _name: String = db
        .query_row(
            "SELECT name FROM exchanges WHERE id = ?1",
            params![exchange_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Exchange not found: {e}"))?;
    drop(db);

    // Return common trading pairs as placeholders. A real implementation
    // would query the exchange's public symbols/markets endpoint.
    Ok(vec![
        "BTC/USDT".into(),
        "ETH/USDT".into(),
        "BNB/USDT".into(),
        "SOL/USDT".into(),
        "XRP/USDT".into(),
        "ADA/USDT".into(),
        "DOGE/USDT".into(),
        "AVAX/USDT".into(),
        "DOT/USDT".into(),
        "MATIC/USDT".into(),
        "LINK/USDT".into(),
        "UNI/USDT".into(),
        "ATOM/USDT".into(),
        "LTC/USDT".into(),
        "FIL/USDT".into(),
        "ETH/BTC".into(),
        "BNB/BTC".into(),
        "SOL/BTC".into(),
        "XRP/BTC".into(),
        "ADA/BTC".into(),
    ])
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

    let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|b| b.as_ref()).collect();

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
fn get_trade_stats(filters: TradeFilters, state: State<'_, AppState>) -> Result<TradeStats, String> {
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
    let expectancy = if total > 0.0 {
        total_pnl / total
    } else {
        0.0
    };
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
    let src = source.unwrap_or_else(|| "live".into());

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
// Settings Commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let settings = state.settings.lock().map_err(|e| format!("Lock: {e}"))?;
    Ok(settings.clone())
}

#[tauri::command]
fn update_settings(
    settings: AppSettings,
    state: State<'_, AppState>,
) -> Result<AppSettings, String> {
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

            let settings = load_settings_from_disk();

            let state = AppState {
                db: Mutex::new(db),
                bot_process: Mutex::new(None),
                settings: Mutex::new(settings),
                bot_logs: Mutex::new(Vec::new()),
            };

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_strategies,
            get_strategy,
            create_strategy,
            save_strategy,
            delete_strategy,
            read_strategy_file,
            run_backtest,
            list_backtests,
            get_backtest,
            save_backtest,
            delete_backtest,
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
            get_settings,
            update_settings,
            detect_python,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
