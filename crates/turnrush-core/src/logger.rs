//! Logger de sessão do TurnRush — o "script de tudo".
//!
//! Escreve, para cada sessão, DOIS arquivos em um diretório fácil (por
//! padrão `$HOME/TurnRush/logs/`):
//!
//! - `session-YYYYMMDD-HHMMSS-<8hex>.log`   → formato humano (uma linha por
//!   evento, com tempo decorrido, nível, evento, mensagem e pares chave=valor);
//! - `session-YYYYMMDD-HHMMSS-<8hex>.jsonl` → eventos estruturados JSON por
//!   linha (`ts`, `level`, `event`, `msg`, `data`).
//!
//! Custo baixíssimo: os eventos são **bufferizados em memória** e descarregados
//! em disco quando o buffer passa de ~8 KiB, a cada 500 ms (thread de flush) e
//! no encerramento limpo. O jogo nunca espera disco no caminho de combate.
//!
//! Extras:
//! - Cabeçalho de sessão com versão da engine, timestamp, seed RNG, SO/kernel,
//!   arquitetura, host e contagem de CPUs.
//! - **Panic hook global**: captura panics e escreve `crash-*.log` com mensagem,
//!   backtrace e as últimas ~200 linhas da sessão.
//! - **Captura de sinais (unix)**: `SIGINT`/`SIGTERM`/`SIGHUP` (Ctrl+C, `kill`,
//!   terminal fechado) registram `session_interrupted` com o sinal, tempo
//!   decorrido e contagem de eventos antes de sair com código 128+sig — a
//!   sessão nunca termina sem explicação.
//! - Rotação: mantém só as últimas N sessões (padrão 20; `TURNRUSH_LOG_KEEP`).
//!
//! Variáveis de ambiente:
//! - `TURNRUSH_LOG_DIR`   → sobrescreve o diretório de logs.
//! - `TURNRUSH_LOG_LEVEL` → filtra por nível (`debug|info|warn|error`).
//! - `TURNRUSH_LOG_KEEP`  → quantas sessões manter (0 desativa a rotação).

use std::backtrace::Backtrace;
use std::collections::VecDeque;
use std::fmt::Write as FmtWrite;
use std::fs::{self, OpenOptions};
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Sessões mantidas por padrão (rotação).
pub const KEEP_DEFAULT: usize = 20;
/// Linhas de sessão preservadas em memória para o relatório de crash.
pub const TAIL_LINES: usize = 200;
const FLUSH_THRESHOLD: usize = 8 * 1024;

// ─────────────────────────────────────────────────────────────────────────────
// Níveis
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    #[inline]
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Debug => "debug",
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Error => "error",
        }
    }

    #[inline]
    pub fn upper(self) -> &'static str {
        match self {
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }

    pub fn parse(s: &str) -> Option<Level> {
        match s.trim().to_ascii_lowercase().as_str() {
            "debug" => Some(Level::Debug),
            "info" => Some(Level::Info),
            "warn" | "warning" => Some(Level::Warn),
            "error" => Some(Level::Error),
            _ => None,
        }
    }

    fn from_env() -> Level {
        std::env::var("TURNRUSH_LOG_LEVEL")
            .ok()
            .and_then(|v| Level::parse(&v))
            .unwrap_or(Level::Info)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Valores estruturados (sem alocação extra nos call sites)
// ─────────────────────────────────────────────────────────────────────────────

/// Valor de um par chave=valor: texto, inteiro, unsigned, bool ou float.
#[derive(Clone, Copy)]
pub enum V<'a> {
    S(&'a str),
    I(i64),
    U(u64),
    B(bool),
    F(f32),
}

impl V<'_> {
    fn write_json(&self, out: &mut String) {
        match self {
            V::S(s) => write_json_str(out, s),
            V::I(i) => {
                let _ = write!(out, "{i}");
            }
            V::U(u) => {
                let _ = write!(out, "{u}");
            }
            V::B(b) => out.push_str(if *b { "true" } else { "false" }),
            V::F(f) => {
                if f.is_finite() {
                    let _ = write!(out, "{f:.3}");
                } else {
                    out.push('0');
                }
            }
        }
    }

    fn write_text(&self, out: &mut String) {
        match self {
            V::S(s) => out.push_str(s),
            V::I(i) => {
                let _ = write!(out, "{i}");
            }
            V::U(u) => {
                let _ = write!(out, "{u}");
            }
            V::B(b) => out.push_str(if *b { "true" } else { "false" }),
            V::F(f) => {
                if f.is_finite() {
                    let _ = write!(out, "{f:.3}");
                } else {
                    out.push('0');
                }
            }
        }
    }
}

/// Escapa uma string como valor JSON (aspas, barra invertida e controles).
pub fn write_json_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

// ─────────────────────────────────────────────────────────────────────────────
// Tempo (UTC, algoritmo de Hinnant — zero dependências)
// ─────────────────────────────────────────────────────────────────────────────

pub fn unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

/// Segundos da era Unix → (ano, mês, dia, hora, min, seg) em UTC.
pub fn civil_from_unix(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let h = (rem / 3600) as u32;
    let mi = ((rem % 3600) / 60) as u32;
    let s = (rem % 60) as u32;

    // Howard Hinnant, "civil_from_days"
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d, h, mi, s)
}

/// Carimbo para nomes de arquivo: `YYYYMMDD-HHMMSS`.
pub fn stamp_file(secs: i64) -> String {
    let (y, mo, d, h, mi, s) = civil_from_unix(secs);
    format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}")
}

/// Carimbo legível: `YYYY-MM-DD HH:MM:SS`.
pub fn stamp_iso(secs: i64) -> String {
    let (y, mo, d, h, mi, s) = civil_from_unix(secs);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

// ─────────────────────────────────────────────────────────────────────────────
// Ambiente
// ─────────────────────────────────────────────────────────────────────────────

fn read_first_line(path: &str) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn kernel_version() -> String {
    read_first_line("/proc/sys/kernel/osrelease").unwrap_or_else(|| std::env::consts::OS.into())
}

fn hostname() -> String {
    read_first_line("/proc/sys/kernel/hostname")
        .or_else(|| std::env::var("HOSTNAME").ok())
        .filter(|h| !h.trim().is_empty())
        .unwrap_or_else(|| "desconhecido".into())
}

fn cpu_count() -> u64 {
    std::thread::available_parallelism().map(|n| n.get() as u64).unwrap_or(0)
}

fn resolve_dir(explicit: Option<&Path>) -> PathBuf {
    if let Some(d) = explicit {
        return d.to_path_buf();
    }
    if let Ok(v) = std::env::var("TURNRUSH_LOG_DIR") {
        if !v.trim().is_empty() {
            return PathBuf::from(v);
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.trim().is_empty() {
            return Path::new(&home).join("TurnRush").join("logs");
        }
    }
    PathBuf::from("./turnrush-logs")
}

fn keep_from_env() -> usize {
    std::env::var("TURNRUSH_LOG_KEEP")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(KEEP_DEFAULT)
}

// ─────────────────────────────────────────────────────────────────────────────
// Logger global
// ─────────────────────────────────────────────────────────────────────────────

struct Inner {
    start: Instant,
    dir: PathBuf,
    text_path: PathBuf,
    jsonl_path: PathBuf,
    text: String,
    json: String,
    tail: VecDeque<String>,
    events: u64,
    flush_errors: u64,
}

pub struct Logger {
    level: AtomicU8,
    inner: Mutex<Inner>,
}

static LOG: Mutex<Option<Arc<Logger>>> = Mutex::new(None);
static HOOK_ONCE: Once = Once::new();
static FLUSHER_ONCE: Once = Once::new();
#[cfg(unix)]
static SIGNAL_ONCE: Once = Once::new();

/// Informações da sessão criada por `init`/`init_cfg`.
#[derive(Clone, Debug)]
pub struct SessionInfo {
    pub dir: PathBuf,
    pub text: PathBuf,
    pub jsonl: PathBuf,
    pub id: String,
}

/// Configuração explícita (usada por testes e embeds; `init` usa o ambiente).
#[derive(Clone, Debug, Default)]
pub struct LoggerCfg {
    pub dir: Option<PathBuf>,
    pub seed: u64,
    pub level: Option<Level>,
    pub keep: Option<usize>,
}

/// Inicializa a sessão de logs com valores do ambiente.
pub fn init(seed: u64) -> Result<SessionInfo, String> {
    init_cfg(LoggerCfg { seed, ..LoggerCfg::default() })
}

/// Inicializa a sessão de logs com configuração explícita.
///
/// Cria o diretório, aplica a rotação, escreve o cabeçalho da sessão nos dois
/// arquivos, instala o panic hook (uma vez) e a thread de flush periódico.
pub fn init_cfg(cfg: LoggerCfg) -> Result<SessionInfo, String> {
    let dir = resolve_dir(cfg.dir.as_deref());
    fs::create_dir_all(&dir)
        .map_err(|e| format!("logger: não consegui criar o diretório {:?}: {e}", dir))?;
    let level = cfg.level.unwrap_or_else(Level::from_env);
    let keep = cfg.keep.unwrap_or_else(keep_from_env);
    rotate(&dir, keep);

    let secs = unix_secs();
    let tag = (unix_nanos() as u64) ^ cfg.seed;
    let id = format!("{}-{:08x}", stamp_file(secs), tag as u32);
    let text_path = dir.join(format!("session-{id}.log"));
    let jsonl_path = dir.join(format!("session-{id}.jsonl"));

    let header = build_header_text(&id, secs, cfg.seed, level);
    let first_json = build_session_start_json(&id, secs, cfg.seed, level);
    append_file(&text_path, &header).map_err(|e| format!("logger: escrever {text_path:?}: {e}"))?;
    append_file(&jsonl_path, &first_json).map_err(|e| format!("logger: escrever {jsonl_path:?}: {e}"))?;

    let mut tail = VecDeque::with_capacity(TAIL_LINES);
    for line in header.lines() {
        tail.push_back(line.to_string());
    }

    let logger = Arc::new(Logger {
        level: AtomicU8::new(level as u8),
        inner: Mutex::new(Inner {
            start: Instant::now(),
            dir,
            text_path: text_path.clone(),
            jsonl_path: jsonl_path.clone(),
            text: String::new(),
            json: String::new(),
            tail,
            events: 0,
            flush_errors: 0,
        }),
    });
    let info = SessionInfo {
        dir: logger.inner.lock().map(|g| g.dir.clone()).unwrap_or_default(),
        text: text_path,
        jsonl: jsonl_path,
        id,
    };
    if let Ok(mut slot) = LOG.lock() {
        *slot = Some(logger);
    }
    HOOK_ONCE.call_once(install_panic_hook);
    FLUSHER_ONCE.call_once(spawn_flusher);
    #[cfg(unix)]
    SIGNAL_ONCE.call_once(crate::logger::sinais::install);
    Ok(info)
}

/// Logger global corrente (se a sessão foi inicializada).
pub fn global() -> Option<Arc<Logger>> {
    LOG.lock().ok().and_then(|slot| slot.clone())
}

/// true se uma sessão de logs está ativa.
pub fn enabled() -> bool {
    LOG.lock().map(|slot| slot.is_some()).unwrap_or(false)
}

/// Evento simples (sem pares) no logger global — no-op se não inicializado.
pub fn event(level: Level, event: &str, msg: &str) {
    if let Some(l) = global() {
        l.event(level, event, msg);
    }
}

/// Evento com pares chave=valor no logger global — no-op se não inicializado.
pub fn kv(level: Level, event: &str, msg: &str, pairs: &[(&str, V<'_>)]) {
    if let Some(l) = global() {
        l.kv(level, event, msg, pairs);
    }
}

/// Descarrega os buffers pendentes no logger global.
pub fn flush() {
    if let Some(l) = global() {
        l.flush();
    }
}

/// Registra o encerramento limpo (tempo total) e descarrega tudo.
pub fn shutdown() {
    if let Some(l) = global() {
        l.shutdown();
    }
}

/// Contagem de eventos registrados na sessão corrente.
pub fn event_count() -> u64 {
    match global() {
        Some(l) => l.inner.lock().map(|g| g.events).unwrap_or(0),
        None => 0,
    }
}

impl Logger {
    /// Evento simples.
    pub fn event(&self, level: Level, event: &str, msg: &str) {
        self.kv(level, event, msg, &[]);
    }

    /// Evento com pares chave=valor (texto: `k=v | k2=v2`; JSON: objeto `data`).
    pub fn kv(&self, level: Level, event: &str, msg: &str, pairs: &[(&str, V<'_>)]) {
        if (level as u8) < self.level.load(Ordering::Relaxed) {
            return;
        }
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        let elapsed = guard.start.elapsed().as_secs_f32();

        let mut line = String::with_capacity(96 + msg.len() + pairs.len() * 16);
        let _ = write!(line, "[{:>9.3}s] {:<5} {}: {}", elapsed, level.upper(), event, msg);
        for (k, v) in pairs {
            line.push_str(" | ");
            line.push_str(k);
            line.push('=');
            v.write_text(&mut line);
        }
        line.push('\n');

        let mut j = String::with_capacity(96 + msg.len() + pairs.len() * 24);
        j.push_str("{\"ts\":\"");
        let _ = write!(j, "{elapsed:.3}");
        j.push_str("\",\"level\":\"");
        j.push_str(level.as_str());
        j.push_str("\",\"event\":");
        write_json_str(&mut j, event);
        j.push_str(",\"msg\":");
        write_json_str(&mut j, msg);
        j.push_str(",\"data\":{");
        for (i, (k, v)) in pairs.iter().enumerate() {
            if i > 0 {
                j.push(',');
            }
            write_json_str(&mut j, k);
            j.push(':');
            v.write_json(&mut j);
        }
        j.push_str("}}\n");

        guard.text.push_str(&line);
        guard.json.push_str(&j);
        guard.tail.push_back(line[..line.len() - 1].to_string());
        while guard.tail.len() > TAIL_LINES {
            guard.tail.pop_front();
        }
        guard.events += 1;

        if guard.text.len() < FLUSH_THRESHOLD && guard.json.len() < FLUSH_THRESHOLD {
            return;
        }
        let text_path = guard.text_path.clone();
        let jsonl_path = guard.jsonl_path.clone();
        let text = std::mem::take(&mut guard.text);
        let json = std::mem::take(&mut guard.json);
        drop(guard);
        let ok = append_file(&text_path, &text).is_ok() && append_file(&jsonl_path, &json).is_ok();
        if !ok {
            if let Ok(mut g) = self.inner.lock() {
                g.flush_errors += 1;
            }
        }
    }

    /// Descarrega os buffers em disco (trava mantida só para trocar buffers).
    pub fn flush(&self) {
        let (text_path, jsonl_path, text, json) = {
            let mut guard = match self.inner.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            if guard.text.is_empty() && guard.json.is_empty() {
                return;
            }
            (
                guard.text_path.clone(),
                guard.jsonl_path.clone(),
                std::mem::take(&mut guard.text),
                std::mem::take(&mut guard.json),
            )
        };
        let ok = append_file(&text_path, &text).is_ok() && append_file(&jsonl_path, &json).is_ok();
        if !ok {
            if let Ok(mut g) = self.inner.lock() {
                g.flush_errors += 1;
            }
        }
    }

    /// Registra `session_end` com o tempo total da sessão e descarrega tudo.
    pub fn shutdown(&self) {
        let (total, events) = {
            let g = match self.inner.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            (g.start.elapsed().as_secs_f32(), g.events)
        };
        self.kv(
            Level::Info,
            "session_end",
            "encerramento limpo da sessão",
            &[("tempo_total_ms", V::U((total * 1000.0) as u64)), ("eventos", V::U(events))],
        );
        self.flush();
    }
}

fn append_file(path: &Path, data: &str) -> std::io::Result<()> {
    if data.is_empty() {
        return Ok(());
    }
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    f.write_all(data.as_bytes())
}

fn spawn_flusher() {
    let _ = std::thread::Builder::new()
        .name("turnrush-log-flush".to_string())
        .spawn(|| loop {
            std::thread::sleep(Duration::from_millis(500));
            if let Some(l) = global() {
                l.flush();
            }
        });
}

// ─────────────────────────────────────────────────────────────────────────────
// Cabeçalho da sessão
// ─────────────────────────────────────────────────────────────────────────────

fn build_header_text(id: &str, secs: i64, seed: u64, level: Level) -> String {
    let mut s = String::with_capacity(512);
    s.push_str("============================================================\n");
    let _ = writeln!(s, " TURNRUSH · SESSÃO {id}");
    s.push_str("============================================================\n");
    let _ = writeln!(s, "engine:      TurnRush v{} (turnrush-core)", crate::ENGINE_VERSION);
    let _ = writeln!(s, "início:      {} UTC", stamp_iso(secs));
    let _ = writeln!(s, "seed RNG:    {seed}");
    let _ = writeln!(s, "SO/kernel:   {}/{}", std::env::consts::OS, kernel_version());
    let _ = writeln!(s, "arquitetura: {}", std::env::consts::ARCH);
    let _ = writeln!(s, "host:        {}", hostname());
    let _ = writeln!(s, "CPUs:        {}", cpu_count());
    let _ = writeln!(s, "log nível:   {}", level.as_str());
    s.push_str("------------------------------------------------------------\n");
    s
}

fn build_session_start_json(id: &str, secs: i64, seed: u64, level: Level) -> String {
    let mut j = String::with_capacity(512);
    j.push_str("{\"ts\":\"0.000\",\"level\":\"info\",\"event\":\"session_start\",");
    j.push_str("\"msg\":\"sessão iniciada\",\"data\":{");
    let pairs: [(&str, V); 10] = [
        ("engine", V::S("turnrush-core")),
        ("version", V::S(crate::ENGINE_VERSION)),
        ("id", V::S(id)),
        ("utc", V::S(&stamp_iso(secs))),
        ("seed", V::U(seed)),
        ("os", V::S(std::env::consts::OS)),
        ("kernel", V::S(&kernel_version())),
        ("arch", V::S(std::env::consts::ARCH)),
        ("host", V::S(&hostname())),
        ("cpus", V::U(cpu_count())),
    ];
    for (i, (k, v)) in pairs.iter().enumerate() {
        if i > 0 {
            j.push(',');
        }
        write_json_str(&mut j, k);
        j.push(':');
        v.write_json(&mut j);
    }
    let _ = write!(j, ",\"log_level\":\"{}\"", level.as_str());
    j.push_str("}}\n");
    j
}

// ─────────────────────────────────────────────────────────────────────────────
// Rotação
// ─────────────────────────────────────────────────────────────────────────────

/// Apaga as sessões mais antigas mantendo `keep` (0 desativa).
fn rotate(dir: &Path, keep: usize) {
    if keep == 0 {
        return;
    }
    let mut ids: Vec<String> = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for entry in rd.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_string) else { continue };
            let Some(rest) = name.strip_prefix("session-") else { continue };
            let Some(id) = rest.strip_suffix(".log") else { continue };
            ids.push(id.to_string());
        }
    }
    if ids.is_empty() {
        return;
    }
    ids.sort();
    let excess = (ids.len() + 1).saturating_sub(keep); // +1 = sessão recém-criada
    for id in ids.into_iter().take(excess) {
        let _ = fs::remove_file(dir.join(format!("session-{id}.log")));
        let _ = fs::remove_file(dir.join(format!("session-{id}.jsonl")));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Panic hook / relatório de crash
// ─────────────────────────────────────────────────────────────────────────────

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info: &std::panic::PanicHookInfo<'_>| {
        default_hook(info);
        write_crash_from_panic(info);
    }));
}

fn panic_message(info: &std::panic::PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "payload de panic não textual".to_string()
    }
}

fn write_crash_from_panic(info: &std::panic::PanicHookInfo<'_>) {
    let Some(l) = global() else { return };
    let msg = panic_message(info);
    let loc = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_else(|| "desconhecido".into());
    let thread = std::thread::current().name().unwrap_or("<sem nome>").to_string();
    let backtrace = Backtrace::force_capture().to_string();
    let (dir, tail) = {
        let guard = match l.inner.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        (guard.dir.clone(), guard.tail.iter().cloned().collect::<Vec<String>>())
    };
    // escreve sem manter a trava do logger
    if let Ok(path) = write_crash_report(&dir, &msg, &loc, &thread, &backtrace, &tail) {
        l.kv(
            Level::Error,
            "panic",
            &msg,
            &[("relatorio", V::S(&path.display().to_string()))],
        );
        l.flush();
    }
}

/// Escreve `crash-YYYYMMDD-HHMMSS.log` com mensagem, backtrace e as últimas
/// linhas da sessão. Pública para testes; em produção chamada pelo panic hook.
pub fn write_crash_report(
    dir: &Path,
    msg: &str,
    location: &str,
    thread: &str,
    backtrace: &str,
    tail: &[String],
) -> std::io::Result<PathBuf> {
    let path = dir.join(format!("crash-{}.log", stamp_file(unix_secs())));
    let mut out = String::with_capacity(4096 + tail.len() * 96);
    out.push_str("============================================================\n");
    out.push_str(" TURNRUSH · RELATÓRIO DE CRASH\n");
    out.push_str("============================================================\n");
    let _ = writeln!(out, "quando:   {} UTC", stamp_iso(unix_secs()));
    let _ = writeln!(out, "pânico:   {msg}");
    let _ = writeln!(out, "thread:   {thread}");
    let _ = writeln!(out, "local:    {location}");
    out.push_str("---------------------- backtrace ---------------------------\n");
    out.push_str(backtrace);
    if !backtrace.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("--------------- últimas linhas da sessão -------------------\n");
    for line in tail.iter().rev().take(TAIL_LINES).collect::<Vec<_>>().iter().rev() {
        out.push_str(line);
        out.push('\n');
    }
    fs::write(&path, out)?;
    Ok(path)
}

// ─────────────────────────────────────────────────────────────────────────────
// Sinais (unix) — Ctrl+C/kill/sessão encerrada não deixam a sessão muda
// ─────────────────────────────────────────────────────────────────────────────

/// Nome legível de um sinal Linux conhecido.
pub fn sig_nome(sig: i32) -> &'static str {
    match sig {
        1 => "SIGHUP",
        2 => "SIGINT",
        13 => "SIGPIPE",
        15 => "SIGTERM",
        _ => "SINAL-DESCONHECIDO",
    }
}

/// Chamado pela thread monitora de sinais: registra `session_interrupted`
/// com o sinal, tempo decorrido e contagem de eventos, descarrega os buffers
/// e encerra o processo com o código convencional `128 + sig`.
#[cfg(unix)]
fn registrar_interrupcao(sig: i32) {
    let codigo = registra_evento_interrupcao(sig);
    std::process::exit(codigo);
}

/// Escreve o evento `session_interrupted` (sem encerrar o processo) e retorna
/// o código de saída convencional `128 + sig`. Separado do exit para testes.
#[cfg(unix)]
fn registra_evento_interrupcao(sig: i32) -> i32 {
    let codigo = 128 + sig;
    let Some(l) = global() else { return codigo };
    let (total, events) = {
        let g = match l.inner.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        (g.start.elapsed().as_secs_f32(), g.events)
    };
    l.kv(
        Level::Info,
        "session_interrupted",
        "sessão interrompida por sinal",
        &[
            ("sinal", V::S(sig_nome(sig))),
            ("sig", V::I(sig as i64)),
            ("tempo_total_ms", V::U((total * 1000.0) as u64)),
            ("eventos", V::U(events)),
            ("exit_code", V::I(codigo as i64)),
        ],
    );
    l.flush();
    codigo
}

#[cfg(unix)]
mod sinais {
    use std::io::Read;
    use std::os::fd::FromRawFd;
    use std::sync::atomic::{AtomicI32, Ordering};

    // FFI mínima contra a libc — o binário Rust já liga contra ela em todo
    // alvo unix, então declaramos só o necessário e mantemos ZERO crates.
    // Nomes escolhidos para não colidir com símbolos de runtime da std
    // (por isso `writev` em vez de `write`; a thread monitora usa std::fs).
    #[repr(C)]
    struct Iovec {
        base: *const u8,
        len: usize,
    }
    unsafe extern "C" {
        fn signal(signum: i32, handler: usize) -> usize;
        fn pipe(fds: *mut i32) -> i32;
        fn writev(fd: i32, iov: *const Iovec, iovcnt: i32) -> isize;
    }

    const SIGHUP: i32 = 1;
    const SIGINT: i32 = 2;
    const SIGTERM: i32 = 15;
    const SIG_ERR: usize = usize::MAX; // (usize)-1

    /// Extremidade de escrita do self-pipe (só sinal-seguro).
    static PIPE_W: AtomicI32 = AtomicI32::new(-1);

    /// Handler: async-signal-safe por design — uma única `writev` de 1 byte no
    /// self-pipe; todo o resto acontece na thread monitora (contexto normal).
    extern "C" fn ao_sinal(sig: i32) {
        let fd = PIPE_W.load(Ordering::Relaxed);
        if fd >= 0 {
            let b = [sig as u8; 1];
            let iov = Iovec { base: b.as_ptr(), len: 1 };
            unsafe { writev(fd, &iov, 1) };
        }
    }

    /// Instala a captura (idempotente, chamada uma vez por processo).
    pub(super) fn install() {
        let mut fds = [0i32; 2];
        if unsafe { pipe(fds.as_mut_ptr()) } != 0 {
            super::kv(
                super::Level::Warn,
                "signal_capture",
                "sem self-pipe; sessão segue sem captura de sinais",
                &[],
            );
            return;
        }
        PIPE_W.store(fds[1], Ordering::Relaxed);
        let mut ok = true;
        let mut instalados = 0u64;
        for s in [SIGHUP, SIGINT, SIGTERM] {
            if unsafe { signal(s, ao_sinal as extern "C" fn(i32) as usize) } == SIG_ERR {
                ok = false;
                break;
            }
            instalados += 1;
        }
        if !ok {
            super::kv(
                super::Level::Warn,
                "signal_capture",
                "signal() falhou; sessão segue sem captura de sinais",
                &[("instalados", super::V::U(instalados))],
            );
            return;
        }
        super::kv(
            super::Level::Info,
            "signal_capture",
            "captura de sinais ativa (SIGINT/SIGTERM/SIGHUP)",
            &[("instalados", super::V::U(instalados))],
        );
        let rd = fds[0];
        let _ = std::thread::Builder::new()
            .name("turnrush-log-signal".to_string())
            .spawn(move || {
                // contexto normal de Rust: leitura via std (sem FFI)
                let mut f = unsafe { std::fs::File::from_raw_fd(rd) };
                let mut b = [0u8; 1];
                // enquanto chegarem bytes do self-pipe, interrompe a sessão
                while let Ok(()) = f.read_exact(&mut b) {
                    super::registrar_interrupcao(b[0] as i32);
                }
                // pipe fechado/erro — encerra a monitora
            });
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Testes
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::Json;
    use std::sync::atomic::AtomicUsize;

    static TEST_LOCK: Mutex<()> = Mutex::new(());
    static TMP_N: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir(tag: &str) -> PathBuf {
        let n = TMP_N.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!(
            "turnrush-log-test-{}-{}-{tag}-{n}",
            std::process::id(),
            unix_nanos() as u64 as u32
        ))
    }

    #[test]
    fn civil_e_carimbos() {
        assert_eq!(civil_from_unix(0), (1970, 1, 1, 0, 0, 0));
        assert_eq!(civil_from_unix(1_700_000_000), (2023, 11, 14, 22, 13, 20));
        assert_eq!(stamp_file(0), "19700101-000000");
        assert_eq!(stamp_iso(1_700_000_000), "2023-11-14 22:13:20");
    }

    #[test]
    fn escape_json() {
        let mut s = String::new();
        write_json_str(&mut s, "a\"b\\c\nd\te");
        assert_eq!(s, "\"a\\\"b\\\\c\\nd\\te\"");
        let mut s2 = String::new();
        write_json_str(&mut s2, "ação ☕");
        assert_eq!(s2, "\"ação ☕\"");
    }

    #[test]
    fn cria_arquivos_cabecalho_e_formato() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = temp_dir("cria");
        let info = init_cfg(LoggerCfg { dir: Some(dir.clone()), seed: 12345, level: Some(Level::Info), keep: Some(0) })
            .expect("init em tmp");
        assert!(info.text.exists() && info.jsonl.exists());

        let l = global().expect("logger global");
        l.event(Level::Info, "teste_evento", "olá mundo");
        l.kv(
            Level::Warn,
            "teste_kv",
            "com pares",
            &[("dano", V::I(42)), ("nome", V::S("Sombra")), ("vivo", V::B(true))],
        );
        l.flush();
        l.shutdown();

        let text = fs::read_to_string(&info.text).unwrap();
        assert!(text.contains("TURNRUSH · SESSÃO"), "cabeçalho ausente:\n{text}");
        assert!(text.contains("engine:      TurnRush v"), "versão ausente");
        assert!(text.contains("seed RNG:    12345"), "seed ausente");
        assert!(text.contains("SO/kernel:"), "kernel ausente");
        assert!(text.contains("arquitetura:"), "arch ausente");
        assert!(text.contains("host:"), "host ausente");
        assert!(text.contains("CPUs:"), "cpus ausente");
        assert!(text.contains("teste_evento: olá mundo"));
        assert!(text.contains("[") && text.contains("] WARN  teste_kv: com pares | dano=42 | nome=Sombra | vivo=true"));
        assert!(text.contains("session_end"));

        let json = fs::read_to_string(&info.jsonl).unwrap();
        let lines: Vec<&str> = json.lines().collect();
        assert!(lines.len() >= 3, "esperadas ≥3 linhas jsonl: {json}");
        let first = crate::json::parse(lines[0]).expect("session_start deve ser JSON válido");
        assert_eq!(first.get("event").and_then(Json::as_str), Some("session_start"));
        assert_eq!(first.get("data").map(|d| d.get("seed")).and_then(|v| v.and_then(Json::as_int)), Some(12345));
        let ev = crate::json::parse(lines[1]).expect("evento deve ser JSON válido");
        assert_eq!(ev.get("event").and_then(Json::as_str), Some("teste_evento"));
        for campo in ["ts", "level", "event", "data"] {
            assert!(ev.get(campo).is_some(), "campo '{campo}' ausente no jsonl");
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn filtro_de_nivel() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = temp_dir("nivel");
        let info = init_cfg(LoggerCfg { dir: Some(dir.clone()), seed: 1, level: Some(Level::Warn), keep: Some(0) })
            .expect("init");
        let l = global().unwrap();
        l.event(Level::Info, "nao_deve_aparecer", "info filtrado");
        l.event(Level::Debug, "debug_tb", "debug filtrado");
        l.event(Level::Warn, "aviso", "warn passa");
        l.event(Level::Error, "erro", "error passa");
        l.flush();

        let text = fs::read_to_string(&info.text).unwrap();
        assert!(!text.contains("nao_deve_aparecer"));
        assert!(!text.contains("debug_tb"));
        assert!(text.contains("aviso"));
        assert!(text.contains("erro"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rotacao_mantem_ultimas_n() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = temp_dir("rot");
        fs::create_dir_all(&dir).unwrap();
        for id in ["20260101-000000-00000001", "20260102-000000-00000002", "20260103-000000-00000003"] {
            fs::write(dir.join(format!("session-{id}.log")), "x").unwrap();
            fs::write(dir.join(format!("session-{id}.jsonl")), "{}\n").unwrap();
        }
        let info = init_cfg(LoggerCfg { dir: Some(dir.clone()), seed: 1, level: Some(Level::Info), keep: Some(2) })
            .expect("init");
        let mut logs: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .filter(|n| n.starts_with("session-") && n.ends_with(".log"))
            .collect();
        logs.sort();
        assert_eq!(logs.len(), 2, "esperadas 2 sessões: {logs:?}");
        assert!(logs[0].contains("00000003"), "deve manter a mais nova das antigas: {logs:?}");
        assert!(logs[1].contains(&info.id), "sessão atual presente");
        assert!(info.text.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn relatorio_de_crash_conteudo() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = temp_dir("crash");
        fs::create_dir_all(&dir).unwrap();
        let tail: Vec<String> = (0..5).map(|i| format!("[     {i}.000s] INFO turno {i}")).collect();
        let path = write_crash_report(
            &dir,
            "boom em teste",
            "src/x.rs:10:5",
            "main",
            "stack backtrace:\n 0: fake_frame",
            &tail,
        )
        .expect("escrever crash");
        let text = fs::read_to_string(&path).unwrap();
        assert!(path.file_name().unwrap().to_str().unwrap().starts_with("crash-"));
        assert!(text.contains("RELATÓRIO DE CRASH"));
        assert!(text.contains("pânico:   boom em teste"));
        assert!(text.contains("thread:   main"));
        assert!(text.contains("local:    src/x.rs:10:5"));
        assert!(text.contains("stack backtrace:"));
        assert!(text.contains("últimas linhas da sessão"));
        assert!(text.contains("[     4.000s] INFO turno 4"), "última linha deve estar no relatório");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn shutdown_registra_tempo_total() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = temp_dir("shutdown");
        let info = init_cfg(LoggerCfg { dir: Some(dir.clone()), seed: 9, level: Some(Level::Debug), keep: Some(0) })
            .expect("init");
        global().unwrap().event(Level::Info, "antes", "x");
        global().unwrap().shutdown();
        let text = fs::read_to_string(&info.text).unwrap();
        assert!(text.contains("session_end"));
        assert!(text.contains("tempo_total_ms="));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn nomes_de_sinais_conhecidos() {
        assert_eq!(sig_nome(2), "SIGINT");
        assert_eq!(sig_nome(15), "SIGTERM");
        assert_eq!(sig_nome(1), "SIGHUP");
        assert_eq!(sig_nome(13), "SIGPIPE");
        assert_eq!(sig_nome(77), "SINAL-DESCONHECIDO");
    }

    #[cfg(unix)]
    #[test]
    fn interrupcao_registra_evento_e_codigo() {
        // Chama registra_evento_interrupcao DIRETAMENTE (sem sinal real e sem
        // exit — o exit acontece só em registrar_interrupcao, que roda na
        // thread monitora do binário). E2E real documentado em docs/LOGS.md.
        let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = temp_dir("interrupt");
        let info = init_cfg(LoggerCfg { dir: Some(dir.clone()), seed: 3, level: Some(Level::Info), keep: Some(0) })
            .expect("init");
        global().unwrap().event(Level::Info, "antes_do_sinal", "x");

        let codigo = registra_evento_interrupcao(2); // SIGINT, sem exit
        assert_eq!(codigo, 130, "código convencional 128+2");

        let text = fs::read_to_string(&info.text).unwrap();
        assert!(text.contains("antes_do_sinal"), "evento pré-sinal presente");
        assert!(text.contains("session_interrupted"), "evento de interrupção presente: {text}");
        assert!(text.contains("sinal=SIGINT"));
        assert!(text.contains("exit_code=130"));

        let json = fs::read_to_string(&info.jsonl).unwrap();
        let linha = json.lines().find(|l| l.contains("session_interrupted")).expect("linha jsonl");
        let ev = crate::json::parse(linha).expect("jsonl válido");
        assert_eq!(ev.get("event").and_then(Json::as_str), Some("session_interrupted"));
        assert_eq!(ev.get("data").map(|d| d.get("sig")).and_then(|v| v.and_then(Json::as_int)), Some(2));
        assert!(linha.contains("tempo_total_ms"), "tempo em ms (contrato inteiro) na linha: {linha}");
        let _ = fs::remove_dir_all(&dir);
    }
}
