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

/// Where the embedded API listens.
pub const API_ADDR: &str = "127.0.0.1:3000";
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
            Status::External => format!("Using the server already running on {API_ADDR}"),
            Status::Running { postgres } => format!(
                "Running on {API_ADDR} · Postgres from {}",
                postgres.display()
            ),
            Status::Failed(error) => format!("Failed — {error}"),
        }
    }
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
        .get(format!("http://{API_ADDR}/api/health"))
        .call()
        .is_ok()
}

/// The running backend, kept so quitting can stop it.
pub struct Backend {
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
    postgres: Held,
}

impl Backend {
    /// Starts everything on a background thread and returns at once; the
    /// progress shows in `status()`.
    pub fn start(root: PathBuf) -> Backend {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let postgres = std::sync::Arc::new(Mutex::new(None));
        let held = postgres.clone();
        set_status(Status::Starting("checking for a running server".into()));
        let thread = std::thread::Builder::new()
            .name("fubbik-backend".into())
            .spawn(move || {
                if let Err(error) = run_backend(root, held, rx) {
                    eprintln!("fubbik: backend: {error}");
                    set_status(Status::Failed(error));
                }
            })
            .expect("spawn backend thread");
        Backend {
            shutdown: Some(tx),
            thread: Some(thread),
            postgres,
        }
    }

    /// Stops the API (draining requests, up to ~15 s), then Postgres.
    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
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
        if !matches!(status(), Status::Failed(_)) {
            set_status(Status::Off);
        }
    }
}

type Held = std::sync::Arc<Mutex<Option<(Binaries, Cluster, Option<Child>)>>>;

fn run_backend(
    root: PathBuf,
    held: Held,
    shutdown: tokio::sync::oneshot::Receiver<()>,
) -> Result<(), String> {
    if api_is_up() {
        set_status(Status::External);
        return Ok(());
    }
    set_status(Status::Starting("finding PostgreSQL".into()));
    let binaries = Binaries::find()?;
    let cluster = Cluster::new(root.clone());
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
            let source = source.trim().to_string();
            set_status(Status::Starting(format!("importing {}", redact(&source))));
            let _ = std::fs::remove_file(cluster.import_request());
            import_database(&binaries, &cluster, &source).await?;
            let _ = std::fs::write(cluster.seeded_marker(), "imported\n");
        }
        create_database(&cluster).await?;
        let url = cluster.url(DB_NAME);
        if !cluster.seeded_marker().exists() {
            set_status(Status::Starting("loading built-in templates".into()));
            seed_core(&url).await?;
            let _ = std::fs::write(cluster.seeded_marker(), "core\n");
        }
        set_status(Status::Starting("starting the API".into()));
        let listener = tokio::net::TcpListener::bind(API_ADDR)
            .await
            .map_err(|e| format!("{API_ADDR} is in use: {e}"))?;
        let options = fubbik_api::server::ServerOptions {
            database_url: url,
            better_auth_secret: secret,
            implicit_dev_session: true,
            features: fubbik_config::Features::default(),
            ai: fubbik_ai::OllamaClient::from_env(),
            cors_origins: fubbik_api::server::parse_cors_origins(&format!(
                "http://localhost:3001,http://{API_ADDR}"
            )),
            web_assets: web_assets(),
        };
        set_status(Status::Running {
            postgres: binaries.bin.clone(),
        });
        fubbik_api::server::serve(options, listener, async {
            let _ = shutdown.await;
        })
        .await
        .map_err(|e| e.to_string())
    })
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
) -> Result<(), String> {
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
    tokio::task::spawn_blocking(move || -> Result<(), String> {
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
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
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
}
