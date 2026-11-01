//! The embedded backend: the fubbik API runs inside this app, on a private
//! PostgreSQL cluster the app owns, so starting the app is all it takes.
//!
//! - Postgres binaries come from a copy bundled in `fubbik.app`, else
//!   Postgres.app, else Homebrew `postgresql@18`. They need pgvector.
//! - The cluster lives in `~/Library/Application Support/fubbik/pgdata` and
//!   listens only on a Unix socket next to it (no TCP port to collide with).
//! - The API serves on 127.0.0.1:3000, so the CLI, MCP and the web app reach
//!   it as usual while the app runs. When something healthy already answers
//!   there (`just dev`), the app uses that and starts nothing.
//! - Quitting stops the API and then Postgres (fast shutdown).

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Where the embedded API listens: 127.0.0.1:3000, or the port in
/// `FUBBIK_DESKTOP_PORT`.
pub fn api_addr() -> &'static str {
    static ADDR: OnceLock<String> = OnceLock::new();
    ADDR.get_or_init(|| {
        let port = std::env::var("FUBBIK_DESKTOP_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(3000);
        format!("127.0.0.1:{port}")
    })
}
/// The port of `api_addr()`.
pub fn api_port() -> u16 {
    api_addr()
        .rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000)
}

/// Names the socket file (`.s.PGSQL.<port>`); there is no TCP listener.
const PG_PORT: u16 = 5432;
const PG_USER: &str = "fubbik";
const DB_NAME: &str = "fubbik";
const PG_MAJOR: &str = "18";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// `backend = "off"` in desktop.toml, or not started (tests).
    Off,
    Starting(String),
    /// Another server answers on :3000; the app uses it.
    External,
    Running {
        postgres: PathBuf,
    },
    Failed(String),
}

impl Status {
    pub fn label(&self) -> String {
        match self {
            Status::Off => "Off — the app does not start a server".into(),
            Status::Starting(step) => format!("Starting — {step}"),
            Status::External => format!("Using the server already running on {}", api_addr()),
            Status::Running { postgres } => format!(
                "Running on {} · Postgres from {}",
                api_addr(),
                postgres.display()
            ),
            Status::Failed(error) => format!("Failed — {error}"),
        }
    }
}

impl Status {
    /// For the sidebar: a short word and whether it is fine, pending or bad.
    pub fn short(&self) -> Option<(&'static str, Health)> {
        Some(match self {
            Status::Off => return None,
            Status::Starting(_) => ("Server starting…", Health::Pending),
            Status::External => ("Server: using a running one", Health::Good),
            Status::Running { .. } => ("Server running", Health::Good),
            Status::Failed(_) => ("Server failed", Health::Bad),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Good,
    Pending,
    Bad,
}

/// The built-in server's own log (the API's tracing output) and Postgres's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFile {
    Server,
    Postgres,
}

impl LogFile {
    pub fn path(self) -> PathBuf {
        data_dir().join(match self {
            LogFile::Server => "backend.log",
            LogFile::Postgres => "postgres.log",
        })
    }
}

/// The last `lines` lines of a log (reads at most the last 512 KB).
pub fn tail(path: &Path, lines: usize) -> Vec<String> {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let len = file.metadata().map_or(0, |m| m.len());
    let from = len.saturating_sub(512 * 1024);
    if file.seek(SeekFrom::Start(from)).is_err() {
        return Vec::new();
    }
    let mut text = String::new();
    let _ = file.take(512 * 1024).read_to_string(&mut text);
    let mut all: Vec<String> = text.lines().map(str::to_string).collect();
    if from > 0 && !all.is_empty() {
        all.remove(0); // a partial first line
    }
    let skip = all.len().saturating_sub(lines);
    all.split_off(skip)
}

/// Sends the in-process API's `tracing` output to `backend.log` (the
/// previous one kept as `backend.log.1` once it passes 5 MB). `RUST_LOG`
/// filters it; the default is info, with sqlx's per-query lines at warn.
fn init_logging(root: &Path) {
    use tracing_subscriber::EnvFilter;
    let _ = std::fs::create_dir_all(root);
    let path = root.join("backend.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 5 * 1024 * 1024) {
        let _ = std::fs::rename(&path, root.join("backend.log.1"));
    }
    let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    else {
        return;
    };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_writer(Mutex::new(file))
        .try_init();
}

fn status_cell() -> &'static Mutex<Status> {
    static STATUS: OnceLock<Mutex<Status>> = OnceLock::new();
    STATUS.get_or_init(|| Mutex::new(Status::Off))
}

pub fn status() -> Status {
    status_cell().lock().unwrap().clone()
}

fn set_status(status: Status) {
    *status_cell().lock().unwrap() = status;
}

/// `~/Library/Application Support/fubbik`, or `FUBBIK_DESKTOP_DATA`.
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("FUBBIK_DESKTOP_DATA") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join("Library/Application Support/fubbik")
}

/// A Postgres installation the app can run: `bin/` with postgres, initdb
/// and pg_ctl, of the right major version, with pgvector installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binaries {
    pub bin: PathBuf,
}

impl Binaries {
    fn tool(&self, name: &str) -> PathBuf {
        self.bin.join(name)
    }

    /// The candidates, best first.
    pub fn candidates() -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        if let Some(dir) = std::env::var_os("FUBBIK_POSTGRES_BIN") {
            dirs.push(PathBuf::from(dir));
        }
        if let Ok(exe) = std::env::current_exe()
            && let Some(contents) = exe.parent().and_then(Path::parent)
        {
            dirs.push(contents.join("Resources/postgres/bin"));
        }
        dirs.push(format!("/Applications/Postgres.app/Contents/Versions/{PG_MAJOR}/bin").into());
        dirs.push("/Applications/Postgres.app/Contents/Versions/latest/bin".into());
        dirs.push(format!("/opt/homebrew/opt/postgresql@{PG_MAJOR}/bin").into());
        dirs.push(format!("/usr/local/opt/postgresql@{PG_MAJOR}/bin").into());
        dirs
    }

    /// The first usable candidate, or why none is.
    pub fn find() -> Result<Binaries, String> {
        let mut reasons = Vec::new();
        for bin in Self::candidates() {
            if !bin.join("postgres").is_file() {
                continue;
            }
            match Self::check(&bin) {
                Ok(found) => return Ok(found),
                Err(reason) => reasons.push(format!("{}: {reason}", bin.display())),
            }
        }
        if reasons.is_empty() {
            return Err(format!(
                "no PostgreSQL {PG_MAJOR} found — install Postgres.app (postgresapp.com), which includes pgvector"
            ));
        }
        Err(reasons.join("; "))
    }

    fn check(bin: &Path) -> Result<Binaries, String> {
        let found = Binaries {
            bin: bin.to_path_buf(),
        };
        for tool in ["postgres", "initdb", "pg_ctl", "pg_config"] {
            if !found.tool(tool).is_file() {
                return Err(format!("{tool} is missing"));
            }
        }
        let version = run(Command::new(found.tool("postgres")).arg("--version"))?;
        if !version
            .split_whitespace()
            .any(|w| w.split('.').next() == Some(PG_MAJOR))
        {
            return Err(format!(
                "needs PostgreSQL {PG_MAJOR}, found {}",
                version.trim()
            ));
        }
        let share = run(Command::new(found.tool("pg_config")).arg("--sharedir"))?;
        if !Path::new(share.trim())
            .join("extension/vector.control")
            .is_file()
        {
            return Err("pgvector is not installed".into());
        }
        Ok(found)
    }
}

fn run(command: &mut Command) -> Result<String, String> {
    let output = command.output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.lines().last().unwrap_or("failed").to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The cluster's files.
#[derive(Debug, Clone)]
pub struct Cluster {
    pub root: PathBuf,
}

impl Cluster {
    pub fn new(root: PathBuf) -> Cluster {
        Cluster { root }
    }
    pub fn data(&self) -> PathBuf {
        self.root.join("pgdata")
    }
    /// `<root>/run`, or a private `/tmp/fubbik-<uid>` when that path would
    /// pass the 103-byte limit on Unix socket paths.
    pub fn socket_dir(&self) -> PathBuf {
        let dir = self.root.join("run");
        let socket_len = dir.as_os_str().len() + format!("/.s.PGSQL.{PG_PORT}").len();
        if socket_len <= 103 {
            return dir;
        }
        use std::os::unix::fs::MetadataExt;
        let uid = std::env::var_os("HOME")
            .and_then(|h| std::fs::metadata(h).ok())
            .map_or(0, |m| m.uid());
        PathBuf::from(format!("/tmp/fubbik-{uid}"))
    }
    fn log(&self) -> PathBuf {
        self.root.join("postgres.log")
    }
    fn seeded_marker(&self) -> PathBuf {
        self.root.join("seeded")
    }
    /// Holds a database URL to import on the next start.
    fn import_request(&self) -> PathBuf {
        self.root.join("import-from")
    }
    fn import_log(&self) -> PathBuf {
        self.root.join("import.log")
    }

    /// The connection string for `database` over the cluster's socket.
    pub fn url(&self, database: &str) -> String {
        format!(
            "postgresql://{PG_USER}@localhost/{database}?host={}&port={PG_PORT}",
            percent_encode(&self.socket_dir().to_string_lossy())
        )
    }

    pub fn initialized(&self) -> bool {
        self.data().join("PG_VERSION").is_file()
    }

    /// `initdb` with ICU collation (what fubbik expects), trust auth for the
    /// app's own user, and settings that keep it off TCP.
    pub fn init(&self, binaries: &Binaries) -> Result<(), String> {
        std::fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        run(Command::new(binaries.tool("initdb"))
            .arg("-D")
            .arg(self.data())
            .args(["-U", PG_USER, "--auth=trust", "-E", "UTF8"])
            .args([
                "--locale-provider=icu",
                "--icu-locale=en-US",
                "--no-instructions",
            ]))
        .map_err(|e| format!("initdb: {e}"))?;
        let socket = self.socket_dir().to_string_lossy().replace('\'', "''");
        let settings = format!(
            "\n# fubbik desktop\nlisten_addresses = ''\nport = {PG_PORT}\nunix_socket_directories = '{socket}'\n"
        );
        let conf = self.data().join("postgresql.conf");
        let mut text = std::fs::read_to_string(&conf).map_err(|e| e.to_string())?;
        text.push_str(&settings);
        std::fs::write(&conf, text).map_err(|e| e.to_string())
    }

    /// True when a postmaster for this cluster is up (from an earlier run).
    fn running(&self, binaries: &Binaries) -> bool {
        Command::new(binaries.tool("pg_ctl"))
            .arg("status")
            .arg("-D")
            .arg(self.data())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    /// Starts `postgres` as a child (None when one was already running) and
    /// waits until its socket accepts connections.
    pub fn start(&self, binaries: &Binaries) -> Result<Option<Child>, String> {
        std::fs::create_dir_all(self.socket_dir()).map_err(|e| e.to_string())?;
        // Only this user may reach the socket (trust auth).
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(self.socket_dir(), std::fs::Permissions::from_mode(0o700));
        if self.running(binaries) {
            return Ok(None);
        }
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log())
            .map_err(|e| e.to_string())?;
        let mut child = Command::new(binaries.tool("postgres"))
            .arg("-D")
            .arg(self.data())
            // The bundle leaves out LLVM; never try to JIT.
            .args(["-c", "jit=off"])
            .stdin(Stdio::null())
            .stdout(log.try_clone().map_err(|e| e.to_string())?)
            .stderr(log)
            .spawn()
            .map_err(|e| format!("postgres: {e}"))?;
        let socket = self.socket_dir().join(format!(".s.PGSQL.{PG_PORT}"));
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if socket.exists() && std::os::unix::net::UnixStream::connect(&socket).is_ok() {
                return Ok(Some(child));
            }
            if let Ok(Some(exit)) = child.try_wait() {
                let log = std::fs::read_to_string(self.log()).unwrap_or_default();
                let reason = log
                    .lines()
                    .rev()
                    .find(|l| l.contains("FATAL"))
                    .unwrap_or("no FATAL line in the log");
                return Err(format!("Postgres exited ({exit}): {reason}"));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(format!(
            "Postgres did not start in 30 s; see {}",
            self.log().display()
        ))
    }

    /// Fast shutdown: active connections are closed, then it exits.
    pub fn stop(&self, binaries: &Binaries) {
        let _ = Command::new(binaries.tool("pg_ctl"))
            .arg("stop")
            .arg("-D")
            .arg(self.data())
            .args(["-m", "fast", "-w", "-t", "20"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

/// Percent-encodes a path for a URL query value.
fn percent_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// The secret that signs session cookies: made once, kept in the data dir.
fn auth_secret(root: &Path) -> Result<String, String> {
    let path = root.join("auth-secret");
    if let Ok(secret) = std::fs::read_to_string(&path)
        && secret.trim().len() >= 32
    {
        return Ok(secret.trim().to_string());
    }
    let mut bytes = [0u8; 32];
    use std::io::Read;
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|e| e.to_string())?;
    let secret: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(&path, &secret).map_err(|e| e.to_string())?;
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    Ok(secret)
}

/// A built web app to serve at `/`: `FUBBIK_WEB_ASSETS_DIR`, the copy in
/// `fubbik.app`, or `apps/web/dist` when run from the repo.
fn web_assets() -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("FUBBIK_WEB_ASSETS_DIR")
        .map(PathBuf::from)
        .into_iter()
        .collect();
    if let Ok(exe) = std::env::current_exe()
        && let Some(contents) = exe.parent().and_then(Path::parent)
    {
        dirs.push(contents.join("Resources/web"));
    }
    dirs.push("apps/web/dist".into());
    dirs.into_iter().find(|d| d.join("index.html").is_file())
}

fn api_is_up() -> bool {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(800)))
        .build()
        .into();
    agent
        .get(format!("http://{}/api/health", api_addr()))
        .call()
        .is_ok()
}

/// Messages to the running backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
    /// Stop serving (quitting).
    Stop,
    /// Stop the API, replace the database with a copy of this one, serve again.
    Import(String),
    /// A backup now, without stopping anything.
    BackupNow,
}

fn control_cell() -> &'static Mutex<Option<tokio::sync::mpsc::UnboundedSender<Control>>> {
    static CONTROL: OnceLock<Mutex<Option<tokio::sync::mpsc::UnboundedSender<Control>>>> =
        OnceLock::new();
    CONTROL.get_or_init(|| Mutex::new(None))
}

/// Sends `control` to the built-in server; false when it is not running
/// here (off, failed, or another server is in use).
pub fn send(control: Control) -> bool {
    matches!(status(), Status::Running { .. })
        && control_cell()
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|tx| tx.send(control).is_ok())
}

fn notices_cell() -> &'static Mutex<Vec<String>> {
    static NOTICES: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    NOTICES.get_or_init(|| Mutex::new(Vec::new()))
}

fn notice(message: impl Into<String>) {
    let message = message.into();
    tracing::info!("{message}");
    notices_cell().lock().unwrap().push(message);
}

/// Messages for the user since the last call (imports, backups).
pub fn take_notices() -> Vec<String> {
    std::mem::take(&mut *notices_cell().lock().unwrap())
}

/// The running backend, kept so quitting can stop it.
pub struct Backend {
    thread: Option<std::thread::JoinHandle<()>>,
    postgres: Held,
}

impl Backend {
    /// Starts everything on a background thread and returns at once; the
    /// progress shows in `status()`.
    pub fn start(root: PathBuf) -> Backend {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        *control_cell().lock().unwrap() = Some(tx);
        let postgres = std::sync::Arc::new(Mutex::new(None));
        let held = postgres.clone();
        init_logging(&root);
        set_status(Status::Starting("checking for a running server".into()));
        let thread = std::thread::Builder::new()
            .name("fubbik-backend".into())
            .spawn(move || {
                if let Err(error) = run_backend(root, held, rx) {
                    tracing::error!("built-in server: {error}");
                    eprintln!("fubbik: backend: {error}");
                    set_status(Status::Failed(error));
                }
            })
            .expect("spawn backend thread");
        Backend {
            thread: Some(thread),
            postgres,
        }
    }

    /// Stops the API (draining requests, up to ~15 s), then Postgres.
    pub fn stop(&mut self) {
        if let Some(tx) = control_cell().lock().unwrap().take() {
            let _ = tx.send(Control::Stop);
        }
        if let Some(thread) = self.thread.take() {
            let deadline = Instant::now() + Duration::from_secs(15);
            while !thread.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        if let Some((binaries, cluster, child)) = self.postgres.lock().unwrap().take() {
            cluster.stop(&binaries);
            if let Some(mut child) = child {
                let _ = child.wait();
            }
        }
        discovery::remove();
        if !matches!(status(), Status::Failed(_)) {
            set_status(Status::Off);
        }
    }
}

type Held = std::sync::Arc<Mutex<Option<(Binaries, Cluster, Option<Child>)>>>;

/// Starts `<this app> --serve` detached, so the server outlives the app,
/// and waits (up to 30 s) until it answers.
pub fn spawn_background() -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Command::new(exe)
        .arg("--serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if api_is_up() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err("the background server did not answer within 30 s".into())
}

/// Stops a background server (`--serve`) that `server.json` names.
pub fn stop_background() -> Result<(), String> {
    let pid = discovery::owner_pid().ok_or("no background server is running")?;
    if pid == std::process::id() {
        return Err("the server is this app's own".into());
    }
    run(Command::new("kill").args(["-TERM", &pid.to_string()])).map(|_| ())
}

/// Refuses a data folder made by another PostgreSQL major version: its files
/// only open with that version (`pg_upgrade`, or a dump and restore, moves it).
fn check_data_version(cluster: &Cluster) -> Result<(), String> {
    let Ok(version) = std::fs::read_to_string(cluster.data().join("PG_VERSION")) else {
        return Ok(());
    };
    let version = version.trim();
    if version == PG_MAJOR {
        return Ok(());
    }
    Err(format!(
        "{} was created by PostgreSQL {version}, but this app runs PostgreSQL {PG_MAJOR}. \
         Back it up with that version's pg_dump, move the folder aside, restart, and import the dump \
         (or run pg_upgrade) — see the desktop README, \"Built-in server\"",
        cluster.data().display()
    ))
}

fn run_backend(
    root: PathBuf,
    held: Held,
    control: tokio::sync::mpsc::UnboundedReceiver<Control>,
) -> Result<(), String> {
    if api_is_up() {
        set_status(Status::External);
        return Ok(());
    }
    set_status(Status::Starting("finding PostgreSQL".into()));
    let binaries = Binaries::find()?;
    let cluster = Cluster::new(root.clone());
    check_data_version(&cluster)?;
    if !cluster.initialized() {
        set_status(Status::Starting("creating the database (first run)".into()));
        cluster.init(&binaries)?;
    }
    set_status(Status::Starting("starting PostgreSQL".into()));
    let child = cluster.start(&binaries)?;
    *held.lock().unwrap() = Some((binaries.clone(), cluster.clone(), child));
    let secret = auth_secret(&root)?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("fubbik-api")
        .build()
        .map_err(|e| e.to_string())?;
    runtime.block_on(async {
        if let Ok(source) = std::fs::read_to_string(cluster.import_request()) {
            let _ = std::fs::remove_file(cluster.import_request());
            run_import(&binaries, &cluster, source.trim()).await?;
        }
        create_database(&cluster).await?;
        let url = cluster.url(DB_NAME);
        if !cluster.seeded_marker().exists() {
            set_status(Status::Starting("loading built-in templates".into()));
            seed_core(&url).await?;
            let _ = std::fs::write(cluster.seeded_marker(), "core\n");
        }
        tokio::spawn(backup::daily(binaries.clone(), cluster.clone()));

        let control = std::sync::Arc::new(tokio::sync::Mutex::new(control));
        loop {
            set_status(Status::Starting("starting the API".into()));
            let listener = bind_api().await?;
            let options = fubbik_api::server::ServerOptions {
                database_url: url.clone(),
                better_auth_secret: secret.clone(),
                implicit_dev_session: true,
                features: fubbik_config::Features::default(),
                ai: fubbik_ai::OllamaClient::from_env(),
                cors_origins: fubbik_api::server::parse_cors_origins(&format!(
                    "http://localhost:3001,http://{}",
                    api_addr()
                )),
                web_assets: web_assets(),
            };
            set_status(Status::Running {
                postgres: binaries.bin.clone(),
            });
            discovery::write();
            // Serve until a message that needs the API stopped arrives.
            let next = std::sync::Arc::new(Mutex::new(None));
            let (until, rx, b, c) = (
                next.clone(),
                control.clone(),
                binaries.clone(),
                cluster.clone(),
            );
            fubbik_api::server::serve(options, listener, async move {
                let mut rx = rx.lock().await;
                loop {
                    match rx.recv().await {
                        Some(Control::BackupNow) => {
                            let (b, c) = (b.clone(), c.clone());
                            tokio::spawn(async move { backup::now(&b, &c).await });
                        }
                        message => {
                            *until.lock().unwrap() = message;
                            break;
                        }
                    }
                }
            })
            .await
            .map_err(|e| e.to_string())?;
            discovery::remove();
            let next = next.lock().unwrap().take();
            match next {
                Some(Control::Import(source)) => {
                    if let Err(error) = run_import(&binaries, &cluster, &source).await {
                        notice(format!("Import failed: {error}"));
                    }
                }
                _ => return Ok(()),
            }
        }
    })
}

/// Binds the API's address, waiting out a just-closed listener.
async fn bind_api() -> Result<tokio::net::TcpListener, String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match tokio::net::TcpListener::bind(api_addr()).await {
            Ok(listener) => return Ok(listener),
            Err(_) if Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(200)).await
            }
            Err(error) => return Err(format!("{} is in use: {error}", api_addr())),
        }
    }
}

/// An import with status and a notice of the outcome.
async fn run_import(binaries: &Binaries, cluster: &Cluster, source: &str) -> Result<(), String> {
    set_status(Status::Starting(format!("importing {}", redact(source))));
    let skipped = import_database(binaries, cluster, source).await?;
    let _ = std::fs::write(cluster.seeded_marker(), "imported\n");
    notice(match skipped {
        0 => format!("Imported {}", redact(source)),
        n => format!(
            "Imported {}; skipped {n} object(s) this Postgres cannot create (the AGE graph) — see import.log",
            redact(source)
        ),
    });
    Ok(())
}

/// `server.json`: tells the CLI and MCP where the built-in server is while
/// it runs (`fubbik_cli::config::discovered_server` reads it).
pub mod discovery {
    use std::path::PathBuf;

    pub fn path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("fubbik/server.json"))
    }

    pub fn write() {
        let Some(path) = path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let body = serde_json::json!({
            "url": format!("http://{}", super::api_addr()),
            "pid": std::process::id(),
            "owner": "fubbik-desktop",
        });
        let _ = std::fs::write(path, body.to_string());
    }

    /// The pid in `server.json`, when the desktop app wrote it.
    pub fn owner_pid() -> Option<u32> {
        let text = std::fs::read_to_string(path()?).ok()?;
        let value: serde_json::Value = serde_json::from_str(&text).ok()?;
        (value["owner"] == "fubbik-desktop")
            .then(|| value["pid"].as_u64())?
            .map(|p| p as u32)
    }

    /// Removes the file if this process wrote it.
    pub fn remove() {
        if owner_pid() == Some(std::process::id())
            && let Some(path) = path()
        {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Backups of the built-in database: one a day (the last 7 kept) in
/// `<data>/backups`, and on demand.
pub mod backup {
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    use super::{Binaries, Cluster, DB_NAME, notice};

    const KEEP: usize = 7;
    const EVERY: Duration = Duration::from_secs(24 * 60 * 60);

    pub fn dir(cluster: &Cluster) -> PathBuf {
        cluster.root.join("backups")
    }

    /// `fubbik-YYYYMMDD-HHMMSS.dump` (UTC), so names sort by time.
    pub fn file_name(at: SystemTime) -> String {
        let secs = at
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let (days, rest) = (secs / 86_400, secs % 86_400);
        // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
        let z = days as i64 + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        format!(
            "fubbik-{year:04}{month:02}{day:02}-{:02}{:02}{:02}.dump",
            rest / 3600,
            rest % 3600 / 60,
            rest % 60
        )
    }

    /// The backups, newest first.
    pub fn list(cluster: &Cluster) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir(cluster))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "dump"))
            .collect();
        files.sort();
        files.reverse();
        files
    }

    /// Keeps the newest `KEEP`.
    pub fn prune(cluster: &Cluster) {
        for old in list(cluster).into_iter().skip(KEEP) {
            let _ = std::fs::remove_file(old);
        }
    }

    /// A `pg_dump -Fc` of the database; the path on success.
    pub async fn create(binaries: &Binaries, cluster: &Cluster) -> Result<PathBuf, String> {
        // One at a time: the daily one and "Back up now" can coincide.
        static RUNNING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
        let _one = RUNNING.lock().await;
        let dir = dir(cluster);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut path = dir.join(file_name(SystemTime::now()));
        for n in 1.. {
            if !path.exists() {
                break;
            }
            let name = file_name(SystemTime::now()).replace(".dump", &format!("-{n}.dump"));
            path = dir.join(name);
        }
        let partial = path.with_extension("dump.partial");
        let output = tokio::process::Command::new(binaries.bin.join("pg_dump"))
            .args(["-Fc", "--no-owner", "-d", &cluster.url(DB_NAME), "-f"])
            .arg(&partial)
            .output()
            .await
            .map_err(|e| format!("pg_dump: {e}"))?;
        if !output.status.success() {
            let _ = std::fs::remove_file(&partial);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(stderr
                .lines()
                .last()
                .unwrap_or("pg_dump failed")
                .to_string());
        }
        std::fs::rename(&partial, &path).map_err(|e| e.to_string())?;
        prune(cluster);
        Ok(path)
    }

    /// "Back up now": a backup and a notice with where it went.
    pub async fn now(binaries: &Binaries, cluster: &Cluster) {
        match create(binaries, cluster).await {
            Ok(path) => notice(format!("Backed up to {}", path.display())),
            Err(error) => notice(format!("Backup failed: {error}")),
        }
    }

    /// Whether the newest backup is older than a day (or there is none).
    pub fn due(cluster: &Cluster) -> bool {
        list(cluster)
            .first()
            .and_then(|p| p.metadata().ok()?.modified().ok())
            .is_none_or(|at| SystemTime::now().duration_since(at).unwrap_or_default() >= EVERY)
    }

    /// Checks hourly and backs up when one is due.
    pub async fn daily(binaries: Binaries, cluster: Cluster) {
        loop {
            if due(&cluster)
                && let Err(error) = create(&binaries, &cluster).await
            {
                tracing::warn!("daily backup failed: {error}");
            }
            tokio::time::sleep(Duration::from_secs(60 * 60)).await;
        }
    }
}

/// The database `fubbik serve` uses by default (Postgres.app, compose).
pub const DEFAULT_IMPORT_SOURCE: &str = "postgresql://localhost:5432/fubbik";

/// Queues an import of `source` (a PostgreSQL URL, only read) for the next
/// start: the built-in database is then replaced by a copy of it.
pub fn request_import(root: &Path, source: &str) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    std::fs::write(root.join("import-from"), source).map_err(|e| e.to_string())
}

/// The URL without a password, for messages.
fn redact(url: &str) -> String {
    match (url.find("://"), url.rfind('@')) {
        (Some(scheme), Some(at)) if at > scheme => {
            let credentials = &url[scheme + 3..at];
            let user = credentials.split(':').next().unwrap_or("");
            format!("{}{user}{}", &url[..scheme + 3], &url[at..])
        }
        _ => url.to_string(),
    }
}

/// Replaces the built-in database with `pg_dump` of `source`. Objects the
/// bundled Postgres cannot create (Apache AGE's graph) are skipped; the
/// count of skipped objects and pg_restore's messages go to import.log.
async fn import_database(
    binaries: &Binaries,
    cluster: &Cluster,
    source: &str,
) -> Result<usize, String> {
    let mut conn = connect_admin(cluster).await?;
    use sqlx::Connection;
    for statement in [
        format!("DROP DATABASE IF EXISTS {DB_NAME} WITH (FORCE)"),
        format!("CREATE DATABASE {DB_NAME}"),
    ] {
        sqlx::query(&statement)
            .execute(&mut conn)
            .await
            .map_err(|e| e.to_string())?;
    }
    conn.close().await.map_err(|e| e.to_string())?;
    let (dump, restore, log) = (
        binaries.tool("pg_dump"),
        binaries.tool("pg_restore"),
        cluster.import_log(),
    );
    let (source, target) = (source.to_string(), cluster.url(DB_NAME));
    tokio::task::spawn_blocking(move || -> Result<usize, String> {
        let log_file = std::fs::File::create(&log).map_err(|e| e.to_string())?;
        let mut dumper = Command::new(&dump)
            .args(["-Fc", "--no-owner", "--no-privileges", "-d", &source])
            .stdout(Stdio::piped())
            .stderr(log_file.try_clone().map_err(|e| e.to_string())?)
            .spawn()
            .map_err(|e| format!("pg_dump: {e}"))?;
        let restored = Command::new(&restore)
            .args(["--no-owner", "--no-privileges", "-d", &target])
            .stdin(dumper.stdout.take().expect("piped stdout"))
            .stderr(log_file)
            .status()
            .map_err(|e| format!("pg_restore: {e}"))?;
        let dumped = dumper.wait().map_err(|e| e.to_string())?;
        let messages = std::fs::read_to_string(&log).unwrap_or_default();
        if !dumped.success() {
            let reason = messages
                .lines()
                .find(|l| l.contains("error"))
                .unwrap_or("see import.log");
            return Err(format!("pg_dump of {} failed: {reason}", redact(&source)));
        }
        // pg_restore exits 1 when it skipped objects ("errors ignored on
        // restore: N"); anything else is a real failure.
        if !restored.success() && !messages.contains("errors ignored on restore") {
            return Err(format!("pg_restore failed; see {}", log.display()));
        }
        Ok(skipped_count(&messages))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// N from pg_restore's "errors ignored on restore: N".
fn skipped_count(messages: &str) -> usize {
    messages
        .lines()
        .find_map(|l| l.split("errors ignored on restore:").nth(1))
        .and_then(|n| n.trim().parse().ok())
        .unwrap_or(0)
}

/// A connection to the cluster's `postgres` database, waiting out startup.
async fn connect_admin(cluster: &Cluster) -> Result<sqlx::PgConnection, String> {
    use sqlx::Connection;
    // The socket opens before startup (or recovery) finishes; until then
    // connections fail with "the database system is starting up".
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match sqlx::PgConnection::connect(&cluster.url("postgres")).await {
            Ok(conn) => return Ok(conn),
            Err(error)
                if Instant::now() < deadline && error.to_string().contains("starting up") =>
            {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            Err(error) => return Err(error.to_string()),
        }
    }
}

/// Creates the `fubbik` database when the cluster only has `postgres`.
async fn create_database(cluster: &Cluster) -> Result<(), String> {
    use sqlx::Connection;
    let mut conn = connect_admin(cluster).await?;
    let exists: Option<i32> = sqlx::query_scalar("SELECT 1 FROM pg_database WHERE datname = $1")
        .bind(DB_NAME)
        .fetch_optional(&mut conn)
        .await
        .map_err(|e| e.to_string())?;
    if exists.is_none() {
        sqlx::query(&format!("CREATE DATABASE {DB_NAME}"))
            .execute(&mut conn)
            .await
            .map_err(|e| e.to_string())?;
    }
    conn.close().await.map_err(|e| e.to_string())
}

/// Built-in chunk types, relations and templates (the `core` seed module).
async fn seed_core(url: &str) -> Result<(), String> {
    use fubbik_db::seed::{ResetPolicy, SeedOptions, SeedScenario};
    let pool = fubbik_db::connect(url).await.map_err(|e| e.to_string())?;
    let options = SeedOptions {
        scenario: SeedScenario::Minimal,
        only: Some(["core".to_string()].into_iter().collect()),
        skip: Default::default(),
        reset: ResetPolicy::None,
    };
    let result = fubbik_db::seed::seed(&pool, &options)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string());
    pool.close().await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_url_reaches_the_socket_with_spaces_encoded() {
        // Given
        let cluster = Cluster::new("/Users/me/Library/Application Support/fubbik".into());

        // When
        let url = cluster.url("fubbik");

        // Then
        assert_eq!(
            url,
            "postgresql://fubbik@localhost/fubbik?host=/Users/me/Library/Application%20Support/fubbik/run&port=5432"
        );
    }

    #[test]
    fn a_long_root_gets_a_short_socket_folder() {
        // Given
        let long = Cluster::new(PathBuf::from(format!("/Users/me/{}", "x".repeat(120))));
        let short = Cluster::new("/Users/me/fubbik".into());

        // When
        let (long_dir, short_dir) = (long.socket_dir(), short.socket_dir());

        // Then
        assert!(
            long_dir.to_string_lossy().starts_with("/tmp/fubbik-"),
            "{}",
            long_dir.display()
        );
        assert_eq!(short_dir, PathBuf::from("/Users/me/fubbik/run"));
    }

    #[test]
    fn backup_names_sort_by_utc_time() {
        // Given: 2026-11-01 12:34:56 UTC and the epoch
        let at = std::time::UNIX_EPOCH + Duration::from_secs(1_793_536_496);

        // When
        let names = [
            backup::file_name(at),
            backup::file_name(std::time::UNIX_EPOCH),
        ];

        // Then
        assert_eq!(
            names,
            ["fubbik-20261101-123456.dump", "fubbik-19700101-000000.dump"]
        );
    }

    #[test]
    fn backups_keep_the_newest_seven() {
        // Given: nine backups
        let root = tempfile::tempdir().unwrap();
        let cluster = Cluster::new(root.path().to_path_buf());
        std::fs::create_dir_all(backup::dir(&cluster)).unwrap();
        for day in 1..=9 {
            std::fs::write(
                backup::dir(&cluster).join(format!("fubbik-202611{day:02}-000000.dump")),
                "",
            )
            .unwrap();
        }

        // When
        backup::prune(&cluster);

        // Then
        let kept = backup::list(&cluster);
        assert_eq!(kept.len(), 7);
        assert!(kept[0].ends_with("fubbik-20261109-000000.dump"));
        assert!(!backup::due(&cluster), "a backup was just written");
    }

    #[test]
    fn skipped_objects_are_counted_from_pg_restore() {
        // Given
        let log = "pg_restore: error: could not execute query\npg_restore: warning: errors ignored on restore: 12\n";

        // When
        let counts = (skipped_count(log), skipped_count("all fine"));

        // Then
        assert_eq!(counts, (12, 0));
    }

    #[test]
    fn a_data_folder_from_another_major_version_is_refused() {
        // Given
        let root = tempfile::tempdir().unwrap();
        let cluster = Cluster::new(root.path().to_path_buf());
        std::fs::create_dir_all(cluster.data()).unwrap();

        // When
        std::fs::write(cluster.data().join("PG_VERSION"), "17\n").unwrap();
        let old = check_data_version(&cluster);
        std::fs::write(cluster.data().join("PG_VERSION"), "18\n").unwrap();
        let current = check_data_version(&cluster);

        // Then
        assert!(old.unwrap_err().contains("PostgreSQL 17"));
        assert!(current.is_ok());
    }

    #[test]
    fn passwords_stay_out_of_messages() {
        // Given
        let urls = [
            "postgresql://me:secret@db:5432/fubbik",
            "postgresql://localhost:5432/fubbik",
        ];

        // When
        let shown: Vec<String> = urls.iter().map(|u| redact(u)).collect();

        // Then
        assert_eq!(
            shown,
            [
                "postgresql://me@db:5432/fubbik",
                "postgresql://localhost:5432/fubbik"
            ]
        );
    }

    #[test]
    fn a_folder_without_postgres_is_not_a_candidate() {
        // Given
        let empty = tempfile::tempdir().unwrap();

        // When
        let checked = Binaries::check(empty.path());

        // Then
        assert!(checked.is_err());
    }

    /// Creates, starts, queries and stops a real cluster when PostgreSQL 18
    /// with pgvector is installed; skipped (passes) otherwise.
    #[test]
    fn a_fresh_cluster_starts_migrates_and_stops() {
        // Given
        let Ok(binaries) = Binaries::find() else {
            eprintln!("no PostgreSQL {PG_MAJOR} with pgvector; skipping");
            return;
        };
        // A short root: the socket path must stay under ~100 bytes.
        let root = tempfile::Builder::new().prefix("fk").tempdir().unwrap();
        let cluster = Cluster::new(root.path().to_path_buf());

        // When
        cluster.init(&binaries).unwrap();
        let child = cluster.start(&binaries).unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let tables: i64 = runtime.block_on(async {
            create_database(&cluster).await.unwrap();
            seed_core(&cluster.url(DB_NAME)).await.unwrap();
            let pool = fubbik_db::connect(&cluster.url(DB_NAME)).await.unwrap();
            let n = sqlx::query_scalar("SELECT count(*) FROM chunk_type")
                .fetch_one(&pool)
                .await
                .unwrap();
            pool.close().await;
            n
        });
        cluster.stop(&binaries);
        let exited = child.map(|mut c| c.wait().is_ok()).unwrap_or(true);

        // Then
        assert!(tables > 0, "core seed loaded chunk types");
        assert!(exited);
        assert!(!cluster.running(&binaries));
    }

    /// The whole lifecycle on port 3107: start, serve, back up, import live,
    /// stop. Needs PostgreSQL 18 with pgvector and a source database:
    /// `FUBBIK_IMPORT_TEST_URL=<url> cargo test full_lifecycle -- --ignored`.
    #[test]
    #[ignore = "starts a real server; needs FUBBIK_IMPORT_TEST_URL"]
    fn full_lifecycle_serves_backs_up_imports_and_stops() {
        // Given
        let source = std::env::var("FUBBIK_IMPORT_TEST_URL").expect("FUBBIK_IMPORT_TEST_URL");
        let config = tempfile::tempdir().unwrap();
        // SAFETY: this test runs alone (ignored by default), before any thread reads these.
        unsafe {
            std::env::set_var("FUBBIK_DESKTOP_PORT", "3107");
            std::env::set_var("XDG_CONFIG_HOME", config.path());
        }
        let root = tempfile::Builder::new().prefix("fk").tempdir().unwrap();
        let wait = |what: &str, secs: u64, done: &dyn Fn() -> bool| {
            let deadline = Instant::now() + Duration::from_secs(secs);
            while !done() {
                assert!(
                    Instant::now() < deadline,
                    "timed out waiting for {what}; status {:?}; notices {:?}",
                    status(),
                    notices_cell().lock().unwrap()
                );
                std::thread::sleep(Duration::from_millis(200));
            }
        };
        let get = |path: &str| -> Option<serde_json::Value> {
            ureq::get(format!("http://127.0.0.1:3107{path}"))
                .call()
                .ok()?
                .body_mut()
                .read_json()
                .ok()
        };
        let cluster = Cluster::new(root.path().to_path_buf());

        // When: it starts
        let mut server = Backend::start(root.path().to_path_buf());
        wait("running", 90, &|| {
            matches!(status(), Status::Running { .. })
        });
        let health = get("/api/health");
        let advertised = discovery::owner_pid();
        // ... backs up (the first daily one, plus one on demand)
        assert!(send(Control::BackupNow));
        wait("two backups", 60, &|| backup::list(&cluster).len() >= 2);
        // ... imports while running
        assert!(send(Control::Import(source.clone())));
        wait("the import notice", 120, &|| {
            notices_cell()
                .lock()
                .unwrap()
                .iter()
                .any(|n| n.starts_with("Imported"))
        });
        wait("running again", 30, &|| {
            matches!(status(), Status::Running { .. }) && get("/api/health").is_some()
        });
        let chunks = get("/api/stats").and_then(|s| s["chunks"].as_i64());
        // ... and stops
        server.stop();

        // Then
        assert_eq!(
            health
                .and_then(|h| h["db"].as_str().map(str::to_string))
                .as_deref(),
            Some("connected")
        );
        assert_eq!(advertised, Some(std::process::id()));
        assert!(chunks.unwrap_or(0) > 0, "imported chunks: {chunks:?}");
        assert_eq!(status(), Status::Off);
        assert_eq!(discovery::owner_pid(), None, "server.json removed");
        assert!(get("/api/health").is_none(), "API stopped");
    }
}
