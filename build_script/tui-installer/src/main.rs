use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use rand::RngCore;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Tabs, Wrap},
    Frame, Terminal,
};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    io::{self, IsTerminal},
    path::Path,
    process::Command,
    time::Duration,
};

const DB_ENGINES: &[&str] = &[
    "PocketBase (Embedded SQLite + Real-time Collections)",
    "SQLite (Standalone Database File)",
    "PostgreSQL (External Server)",
    "MySQL / MariaDB (External Server)",
];

const STORAGE_PROVIDERS: &[&str] = &[
    "Local Disk (/var/lib/signline/storage)",
    "Amazon S3 / S3-Compatible (Ceph / MinIO)",
    "Google Cloud Storage (GCS)",
    "Azure Blob Storage",
];

const EMAIL_TRANSPORTS: &[&str] = &[
    "Gmail OAuth 2.0 (Gmail / Google Workspace)",
    "SMTP Relay Server (STARTTLS / TLS)",
    "Local Event Log Only",
];

#[derive(Clone, Debug)]
struct InstallerConfig {
    db_engine_idx: usize,
    db_connection: String,
    pb_port: String,
    storage_idx: usize,
    storage_path: String,
    email_transport_idx: usize,
    host: String,
    port: String,
    public_url: String,
    data_dir: String,
    org_name: String,
    admin_email: String,
    admin_password: String,
    secret_key_base: String,
    enable_systemd: bool,
}

impl Default for InstallerConfig {
    fn default() -> Self {
        let mut key_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key_bytes);
        let data_dir = env::var("SIGNLINE_DATA_DIR")
            .unwrap_or_else(|_| "/var/lib/signline".to_string());
        Self {
            db_engine_idx: 0,
            db_connection: format!("{}/pb_data", data_dir),
            pb_port: env::var("SIGNLINE_PB_PORT").unwrap_or_else(|_| "8090".to_string()),
            storage_idx: 0,
            storage_path: format!("{}/storage", data_dir),
            email_transport_idx: 0,
            host: env::var("SIGNLINE_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("SIGNLINE_PORT").unwrap_or_else(|_| "3000".to_string()),
            public_url: env::var("SIGNLINE_PUBLIC_URL").unwrap_or_default(),
            data_dir,
            org_name: env::var("SIGNLINE_ORG_NAME")
                .unwrap_or_else(|_| "Signline Organization".to_string()),
            admin_email: env::var("SIGNLINE_ADMIN_EMAIL")
                .unwrap_or_else(|_| "admin@signline.local".to_string()),
            admin_password: String::new(),
            secret_key_base: hex::encode(key_bytes),
            enable_systemd: true,
        }
    }
}

impl InstallerConfig {
    fn load_from_env_file(&mut self, path: &str) {
        if let Ok(contents) = fs::read_to_string(path) {
            for raw in contents.lines() {
                let line = raw.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = line.split_once('=') {
                    let key = k.trim();
                    let val = v.trim().trim_matches('"').trim_matches('\'').to_string();
                    match key {
                        "HOST" => self.host = val,
                        "PORT" => self.port = val,
                        "PUBLIC_URL" => self.public_url = val,
                        "POCKETBASE_DATA_DIR" | "DATABASE_URL" => self.db_connection = val,
                        "STORAGE_DIR" => self.storage_path = val,
                        "SECRET_KEY_BASE" if !val.is_empty() => self.secret_key_base = val,
                        "INITIAL_ADMIN_EMAIL" => self.admin_email = val,
                        "INITIAL_ORG_NAME" => self.org_name = val,
                        "DB_ENGINE" => {
                            if let Some(idx) = DB_ENGINES.iter().position(|e| e.starts_with(&val)) {
                                self.db_engine_idx = idx;
                            }
                        }
                        "STORAGE_PROVIDER" => {
                            if let Some(idx) =
                                STORAGE_PROVIDERS.iter().position(|s| s.starts_with(&val))
                            {
                                self.storage_idx = idx;
                            }
                        }
                        "EMAIL_TRANSPORT" => {
                            if let Some(idx) =
                                EMAIL_TRANSPORTS.iter().position(|t| t.starts_with(&val))
                            {
                                self.email_transport_idx = idx;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct UserSummary {
    id: String,
    email: String,
    #[serde(default)]
    first_name: String,
    #[serde(default)]
    last_name: String,
    #[serde(default)]
    role: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Login,
    Welcome,
    SetupWizard,
    Installing,
    Complete,
    PasswordReset,
    ServiceStatus,
}

struct App {
    screen: Screen,
    active_tab: usize,
    is_root: bool,
    authenticated_user: String,
    // Login screen fields
    login_email: String,
    login_password: String,
    login_field: usize,
    login_error: String,
    // Setup Wizard fields
    config: InstallerConfig,
    selected_field: usize,
    editing: bool,
    install_logs: Vec<String>,
    progress: u16,
    env_output_path: String,
    // Password Reset fields
    users: Vec<UserSummary>,
    selected_user_idx: usize,
    reset_email: String,
    reset_password: String,
    reset_confirm: String,
    reset_field: usize,
    reset_editing: bool,
    reset_status_msg: String,
    reset_status_ok: bool,
    // Service Status fields
    service_status_lines: Vec<String>,
}

fn check_is_root() -> bool {
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("Uid:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                // parts[1] is real UID, parts[2] is effective UID
                if parts.get(2) == Some(&"0") || parts.get(1) == Some(&"0") {
                    return true;
                }
            }
        }
    }
    env::var("USER").map(|u| u == "root").unwrap_or(false)
}

fn find_server_binary() -> String {
    let candidates = [
        "/usr/bin/signline-server",
        "backend/target/release/signline-backend",
        "backend/target/debug/signline-backend",
    ];
    for c in candidates {
        if Path::new(c).exists() {
            return c.to_string();
        }
    }
    "signline-server".to_string()
}

impl App {
    fn new(env_output_path: String, start_in_wizard: bool) -> Self {
        let is_root = check_is_root();
        let mut config = InstallerConfig::default();
        config.load_from_env_file(&env_output_path);

        let initial_screen = if !is_root {
            Screen::Login
        } else if start_in_wizard {
            Screen::Welcome
        } else {
            Screen::SetupWizard
        };

        let mut app = Self {
            screen: initial_screen,
            active_tab: 0,
            is_root,
            authenticated_user: if is_root {
                "root (System Administrator)".to_string()
            } else {
                String::new()
            },
            login_email: String::new(),
            login_password: String::new(),
            login_field: 0,
            login_error: String::new(),
            reset_email: config.admin_email.clone(),
            config,
            selected_field: 0,
            editing: false,
            install_logs: Vec::new(),
            progress: 0,
            env_output_path,
            users: Vec::new(),
            selected_user_idx: 0,
            reset_password: String::new(),
            reset_confirm: String::new(),
            reset_field: 0,
            reset_editing: false,
            reset_status_msg: String::new(),
            reset_status_ok: true,
            service_status_lines: Vec::new(),
        };

        if is_root {
            app.refresh_users();
            app.refresh_service_status();
        }
        app
    }

    fn verify_user_login(&mut self) -> bool {
        let email = self.login_email.trim();
        let password = &self.login_password;
        if email.is_empty() || password.is_empty() {
            self.login_error = "Please enter both email and password.".to_string();
            return false;
        }
        let server_bin = find_server_binary();
        let output = Command::new(&server_bin)
            .args(["--verify-login", email, password])
            .output();
        match output {
            Ok(out) if out.status.success() => {
                self.authenticated_user = email.to_string();
                self.login_error.clear();
                self.login_password.clear();
                self.refresh_users();
                self.refresh_service_status();
                true
            }
            Ok(out) => {
                let err_txt = String::from_utf8_lossy(&out.stderr).trim().to_string();
                self.login_error = if err_txt.is_empty() {
                    "Authentication failed: invalid email or password.".to_string()
                } else {
                    format!("Authentication failed: {}", err_txt)
                };
                false
            }
            Err(e) => {
                self.login_error = format!("Could not invoke {}: {}", server_bin, e);
                false
            }
        }
    }

    fn refresh_users(&mut self) {
        let server_bin = find_server_binary();
        if let Ok(out) = Command::new(&server_bin).arg("--list-users").output() {
            if out.status.success() {
                if let Ok(parsed) = serde_json::from_slice::<Vec<UserSummary>>(&out.stdout) {
                    self.users = parsed;
                    if let Some(first) = self.users.first() {
                        if self.reset_email.is_empty() {
                            self.reset_email = first.email.clone();
                        }
                    }
                }
            }
        }
    }

    fn execute_password_reset(&mut self) {
        let email = self.reset_email.trim().to_string();
        if email.is_empty() {
            self.reset_status_ok = false;
            self.reset_status_msg = "Error: Target user email cannot be empty.".to_string();
            return;
        }
        if self.reset_password.len() < 6 {
            self.reset_status_ok = false;
            self.reset_status_msg =
                "Error: New password must be at least 6 characters.".to_string();
            return;
        }
        if self.reset_password != self.reset_confirm {
            self.reset_status_ok = false;
            self.reset_status_msg =
                "Error: New password and confirmation do not match.".to_string();
            return;
        }

        let server_bin = find_server_binary();
        match Command::new(&server_bin)
            .args(["--reset-password", &email, &self.reset_password])
            .output()
        {
            Ok(out) if out.status.success() => {
                let msg = String::from_utf8_lossy(&out.stdout).trim().to_string();
                self.reset_status_ok = true;
                self.reset_status_msg = if msg.is_empty() {
                    format!("✓ Password successfully reset for {}", email)
                } else {
                    format!("✓ {}", msg)
                };
                self.reset_password.clear();
                self.reset_confirm.clear();
                self.refresh_users();
            }
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                self.reset_status_ok = false;
                self.reset_status_msg = format!("Error: {}", err);
            }
            Err(e) => {
                self.reset_status_ok = false;
                self.reset_status_msg = format!("Failed to run {}: {}", server_bin, e);
            }
        }
    }

    fn refresh_service_status(&mut self) {
        self.service_status_lines.clear();
        let active = Command::new("systemctl")
            .args(["is-active", "signline.service"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        self.service_status_lines
            .push(format!("Systemd Service (signline.service) : {}", active));

        let health = Command::new("curl")
            .args([
                "-s",
                "--max-time",
                "2",
                &format!("http://127.0.0.1:{}/api/health", self.config.port),
            ])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        self.service_status_lines.push(format!(
            "Backend API Health                 : {}",
            if health.is_empty() {
                "unreachable"
            } else {
                &health
            }
        ));
        self.service_status_lines
            .push(format!("Configured Public URL              : {}", self.config.public_url));
        self.service_status_lines.push(format!(
            "Database Engine                    : {}",
            DB_ENGINES[self.config.db_engine_idx]
        ));
        self.service_status_lines.push(format!(
            "Database Connection / Path         : {}",
            self.config.db_connection
        ));
        self.service_status_lines.push(format!(
            "Storage Provider                   : {}",
            STORAGE_PROVIDERS[self.config.storage_idx]
        ));
        self.service_status_lines.push(String::new());
        self.service_status_lines
            .push("--- Recent Service Logs (journalctl -u signline.service) ---".to_string());

        if let Ok(logs) = Command::new("journalctl")
            .args(["-u", "signline.service", "-n", "8", "--no-pager"])
            .output()
        {
            for l in String::from_utf8_lossy(&logs.stdout).lines() {
                self.service_status_lines.push(l.to_string());
            }
        }
    }

    fn wizard_field_count(&self) -> usize {
        13
    }

    fn is_choice_field(&self, idx: usize) -> bool {
        matches!(idx, 0 | 3 | 5 | 12)
    }

    fn cycle_choice_field(&mut self, forward: bool) {
        match self.selected_field {
            0 => {
                if forward {
                    self.config.db_engine_idx = (self.config.db_engine_idx + 1) % DB_ENGINES.len();
                } else {
                    self.config.db_engine_idx =
                        (self.config.db_engine_idx + DB_ENGINES.len() - 1) % DB_ENGINES.len();
                }
                match self.config.db_engine_idx {
                    0 => self.config.db_connection = format!("{}/pb_data", self.config.data_dir),
                    1 => {
                        self.config.db_connection =
                            format!("{}/signline.sqlite", self.config.data_dir)
                    }
                    2 => {
                        self.config.db_connection =
                            "postgres://signline:password@127.0.0.1:5432/signline".to_string()
                    }
                    3 => {
                        self.config.db_connection =
                            "mysql://signline:password@127.0.0.1:3306/signline".to_string()
                    }
                    _ => {}
                }
            }
            3 => {
                if forward {
                    self.config.storage_idx =
                        (self.config.storage_idx + 1) % STORAGE_PROVIDERS.len();
                } else {
                    self.config.storage_idx = (self.config.storage_idx + STORAGE_PROVIDERS.len()
                        - 1)
                        % STORAGE_PROVIDERS.len();
                }
            }
            5 => {
                if forward {
                    self.config.email_transport_idx =
                        (self.config.email_transport_idx + 1) % EMAIL_TRANSPORTS.len();
                } else {
                    self.config.email_transport_idx = (self.config.email_transport_idx
                        + EMAIL_TRANSPORTS.len()
                        - 1)
                        % EMAIL_TRANSPORTS.len();
                }
            }
            12 => {
                self.config.enable_systemd = !self.config.enable_systemd;
            }
            _ => {}
        }
    }

    fn current_wizard_field_mut(&mut self) -> Option<&mut String> {
        match self.selected_field {
            1 => Some(&mut self.config.db_connection),
            2 => Some(&mut self.config.pb_port),
            4 => Some(&mut self.config.storage_path),
            6 => Some(&mut self.config.host),
            7 => Some(&mut self.config.port),
            8 => Some(&mut self.config.public_url),
            9 => Some(&mut self.config.org_name),
            10 => Some(&mut self.config.admin_email),
            11 => Some(&mut self.config.admin_password),
            _ => None,
        }
    }

    fn execute_install_steps(&mut self) {
        self.install_logs.clear();
        self.install_logs
            .push("Creating Signline directories & setting permissions...".to_string());
        self.progress = 20;

        let data_path = Path::new(&self.config.data_dir);
        let pb_data = if self.config.db_engine_idx == 0 {
            Path::new(&self.config.db_connection).to_path_buf()
        } else {
            data_path.join("pb_data")
        };
        let storage_dir = Path::new(&self.config.storage_path).to_path_buf();
        let _ = fs::create_dir_all(&pb_data);
        let _ = fs::create_dir_all(&storage_dir);

        if let Some(parent) = Path::new(&self.env_output_path).parent() {
            let _ = fs::create_dir_all(parent);
        }

        self.install_logs.push(format!(
            "Writing environment configuration (DB: {}, Storage: {}) to {}...",
            DB_ENGINES[self.config.db_engine_idx],
            STORAGE_PROVIDERS[self.config.storage_idx],
            self.env_output_path
        ));
        self.progress = 50;

        let env_contents = format!(
            "# Signline v0.1.0b Configuration (Generated by signline TUI)\n\
             HOST={}\n\
             PORT={}\n\
             PUBLIC_URL={}\n\
             DB_ENGINE=\"{}\"\n\
             DATABASE_URL=\"{}\"\n\
             POCKETBASE_URL=http://127.0.0.1:{}\n\
             POCKETBASE_BIN=/usr/lib/signline/bin/pocketbase\n\
             POCKETBASE_DATA_DIR={}\n\
             STORAGE_PROVIDER=\"{}\"\n\
             STORAGE_DIR={}\n\
             EMAIL_TRANSPORT=\"{}\"\n\
             PUBLIC_DIR=/usr/share/signline/public\n\
             FRONTEND_DIST_DIR=/usr/share/signline/frontend/dist\n\
             SECRET_KEY_BASE={}\n\
             INITIAL_ADMIN_EMAIL={}\n\
             INITIAL_ORG_NAME=\"{}\"\n",
            self.config.host,
            self.config.port,
            self.config.public_url,
            DB_ENGINES[self.config.db_engine_idx],
            self.config.db_connection,
            self.config.pb_port,
            pb_data.display(),
            STORAGE_PROVIDERS[self.config.storage_idx],
            storage_dir.display(),
            EMAIL_TRANSPORTS[self.config.email_transport_idx],
            self.config.secret_key_base,
            self.config.admin_email,
            self.config.org_name,
        );

        let _ = fs::write(&self.env_output_path, env_contents);
        self.progress = 75;

        if self.config.enable_systemd && Path::new("/run/systemd/system").exists() {
            self.install_logs.push(
                "Reloading systemd daemon, enabling and starting signline.service...".to_string(),
            );
            let _ = Command::new("systemctl").arg("daemon-reload").status();
            let _ = Command::new("systemctl")
                .args(["enable", "signline.service"])
                .status();
            let _ = Command::new("systemctl")
                .args(["restart", "signline.service"])
                .status();
        } else {
            self.install_logs.push(
                "Systemd activation skipped or handled by package manager.".to_string(),
            );
        }

        if !self.config.admin_password.is_empty() && !self.config.admin_email.is_empty() {
            let server_bin = find_server_binary();
            if let Ok(out) = Command::new(&server_bin)
                .args([
                    "--reset-password",
                    &self.config.admin_email,
                    &self.config.admin_password,
                ])
                .output()
            {
                if out.status.success() {
                    self.install_logs.push(format!(
                        "Provisioned administrator credentials for {}",
                        self.config.admin_email
                    ));
                }
            }
        }

        self.install_logs
            .push("Signline v0.1.0b installation & configuration complete!".to_string());
        self.progress = 100;
        self.screen = Screen::Complete;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let mut non_interactive = false;
    let mut start_in_wizard = false;
    let mut env_path = env::var("SIGNLINE_ENV_FILE")
        .unwrap_or_else(|_| "/etc/signline/signline.env".to_string());

    if let Some(prog) = args.first() {
        if prog.ends_with("signline-installer") {
            start_in_wizard = true;
        }
    }

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--non-interactive" | "-y" => non_interactive = true,
            "--wizard" | "setup" | "install" => start_in_wizard = true,
            "--env-file" if i + 1 < args.len() => {
                env_path = args[i + 1].clone();
                i += 1;
            }
            "reset-password" | "--reset-password" if i + 2 < args.len() => {
                if !check_is_root() {
                    eprintln!("Error: Non-root users must run `signline` interactively to log in before resetting passwords.");
                    std::process::exit(1);
                }
                let server_bin = find_server_binary();
                let status = Command::new(&server_bin)
                    .args(["--reset-password", &args[i + 1], &args[i + 2]])
                    .status()?;
                std::process::exit(status.code().unwrap_or(0));
            }
            "--help" | "-h" => {
                println!(
                    "Signline v0.1.0b — Open Source Digital Signature Platform (Ratatui TUI Manager & Installer)\n\
                     Usage:\n\
                       signline                                    Launch interactive Ratatui TUI Management & Setup Suite\n\
                       signline setup                              Launch interactive Setup & Installation Wizard\n\
                       signline reset-password <email> <password>  Reset or provision a user's password (root only)\n\
                       signline-installer [--non-interactive]      Run package installation wizard\n\
                     Powered by Signline, based on Signline (https://github.com/signline/signline)"
                );
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    if non_interactive || !io::stdout().is_terminal() {
        let mut app = App::new(env_path, true);
        app.execute_install_steps();
        for line in &app.install_logs {
            println!("[signline] {}", line);
        }
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(env_path, start_in_wizard);
    let res = run_tui(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Signline TUI error: {}", err);
    }

    Ok(())
}

fn run_tui<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| draw_ui(f, app))?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // Global Tab switching when authenticated and not actively typing into a field
                if app.screen != Screen::Login
                    && app.screen != Screen::Welcome
                    && !app.editing
                    && !app.reset_editing
                {
                    match key.code {
                        KeyCode::F(1) | KeyCode::Char('1') => {
                            app.active_tab = 0;
                            app.screen = Screen::SetupWizard;
                            continue;
                        }
                        KeyCode::F(2) | KeyCode::Char('2') => {
                            app.active_tab = 1;
                            app.screen = Screen::PasswordReset;
                            app.refresh_users();
                            continue;
                        }
                        KeyCode::F(3) | KeyCode::Char('3') => {
                            app.active_tab = 2;
                            app.screen = Screen::ServiceStatus;
                            app.refresh_service_status();
                            continue;
                        }
                        _ => {}
                    }
                }

                match app.screen {
                    Screen::Login => match key.code {
                        KeyCode::Esc => return Ok(()),
                        KeyCode::Tab | KeyCode::Down => {
                            app.login_field = (app.login_field + 1) % 2;
                        }
                        KeyCode::Up => {
                            app.login_field = if app.login_field == 0 { 1 } else { 0 };
                        }
                        KeyCode::Backspace => {
                            if app.login_field == 0 {
                                app.login_email.pop();
                            } else {
                                app.login_password.pop();
                            }
                        }
                        KeyCode::Enter => {
                            if app.login_field == 0 && app.login_password.is_empty() {
                                app.login_field = 1;
                            } else if app.verify_user_login() {
                                app.screen = Screen::SetupWizard;
                            }
                        }
                        KeyCode::Char(c) => {
                            if app.login_field == 0 {
                                app.login_email.push(c);
                            } else {
                                app.login_password.push(c);
                            }
                        }
                        _ => {}
                    },
                    Screen::Welcome => match key.code {
                        KeyCode::Enter | KeyCode::Right | KeyCode::Char(' ') => {
                            app.screen = Screen::SetupWizard;
                        }
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        _ => {}
                    },
                    Screen::SetupWizard => {
                        if app.editing {
                            match key.code {
                                KeyCode::Enter | KeyCode::Esc => {
                                    app.editing = false;
                                }
                                KeyCode::Backspace => {
                                    if let Some(field) = app.current_wizard_field_mut() {
                                        field.pop();
                                    }
                                }
                                KeyCode::Char(c) => {
                                    if let Some(field) = app.current_wizard_field_mut() {
                                        field.push(c);
                                    }
                                }
                                _ => {}
                            }
                        } else {
                            match key.code {
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if app.selected_field > 0 {
                                        app.selected_field -= 1;
                                    } else {
                                        app.selected_field = app.wizard_field_count() - 1;
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                                    app.selected_field =
                                        (app.selected_field + 1) % app.wizard_field_count();
                                }
                                KeyCode::Right | KeyCode::Char('l')
                                    if app.is_choice_field(app.selected_field) =>
                                {
                                    app.cycle_choice_field(true);
                                }
                                KeyCode::Left | KeyCode::Char('h')
                                    if app.is_choice_field(app.selected_field) =>
                                {
                                    app.cycle_choice_field(false);
                                }
                                KeyCode::Enter | KeyCode::Char('e') | KeyCode::Char(' ') => {
                                    if app.is_choice_field(app.selected_field) {
                                        app.cycle_choice_field(true);
                                    } else if key.code != KeyCode::Char(' ') {
                                        app.editing = true;
                                    }
                                }
                                KeyCode::Char('i') | KeyCode::F(5) => {
                                    app.screen = Screen::Installing;
                                    app.execute_install_steps();
                                }
                                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                                _ => {}
                            }
                        }
                    }
                    Screen::Installing => {
                        app.execute_install_steps();
                    }
                    Screen::Complete => match key.code {
                        KeyCode::Enter | KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char('m') => {
                            app.screen = Screen::SetupWizard;
                        }
                        _ => {}
                    },
                    Screen::PasswordReset => {
                        if app.reset_editing {
                            match key.code {
                                KeyCode::Enter | KeyCode::Esc => {
                                    app.reset_editing = false;
                                }
                                KeyCode::Backspace => match app.reset_field {
                                    0 => {
                                        app.reset_email.pop();
                                    }
                                    1 => {
                                        app.reset_password.pop();
                                    }
                                    2 => {
                                        app.reset_confirm.pop();
                                    }
                                    _ => {}
                                },
                                KeyCode::Char(c) => match app.reset_field {
                                    0 => app.reset_email.push(c),
                                    1 => app.reset_password.push(c),
                                    2 => app.reset_confirm.push(c),
                                    _ => {}
                                },
                                _ => {}
                            }
                        } else {
                            match key.code {
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if app.reset_field > 0 {
                                        app.reset_field -= 1;
                                    } else {
                                        app.reset_field = 3;
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                                    app.reset_field = (app.reset_field + 1) % 4;
                                }
                                KeyCode::Left | KeyCode::Right => {
                                    if !app.users.is_empty() {
                                        if key.code == KeyCode::Right {
                                            app.selected_user_idx =
                                                (app.selected_user_idx + 1) % app.users.len();
                                        } else {
                                            app.selected_user_idx = (app.selected_user_idx
                                                + app.users.len()
                                                - 1)
                                                % app.users.len();
                                        }
                                        app.reset_email =
                                            app.users[app.selected_user_idx].email.clone();
                                    }
                                }
                                KeyCode::Enter => {
                                    if app.reset_field == 3 {
                                        app.execute_password_reset();
                                    } else {
                                        app.reset_editing = true;
                                    }
                                }
                                KeyCode::Char('r') => {
                                    app.execute_password_reset();
                                }
                                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                                _ => {}
                            }
                        }
                    }
                    Screen::ServiceStatus => match key.code {
                        KeyCode::Char('r') | KeyCode::Enter => {
                            app.refresh_service_status();
                        }
                        KeyCode::Char('s') => {
                            let _ = Command::new("systemctl")
                                .args(["restart", "signline.service"])
                                .status();
                            app.refresh_service_status();
                        }
                        KeyCode::Char('t') => {
                            let _ = Command::new("systemctl")
                                .args(["stop", "signline.service"])
                                .status();
                            app.refresh_service_status();
                        }
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        _ => {}
                    },
                }
            }
        }
    }
}

fn mask_secret(s: &str) -> String {
    if s.is_empty() {
        "(not set)".to_string()
    } else {
        "*".repeat(s.chars().count().min(24))
    }
}

fn draw_ui(f: &mut Frame, app: &App) {
    let size = f.area();

    // Signline brand colors: Primary #2161AD (RGB: 33, 97, 173), Accent #253E8A (RGB: 37, 62, 138)
    let brand_blue = Color::Rgb(33, 97, 173);
    let brand_navy = Color::Rgb(37, 62, 138);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(3),
        ])
        .split(size);

    // Header
    let auth_badge = if app.authenticated_user.is_empty() {
        "Authentication Required".to_string()
    } else {
        format!("User: {}", app.authenticated_user)
    };
    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(brand_blue))
        .title(Span::styled(
            format!(
                " Signline v0.1.0b — Setup & Management Wizard [{}] ",
                auth_badge
            ),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ));
    let header_text = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "Signline ",
                Style::default().fg(brand_blue).add_modifier(Modifier::BOLD),
            ),
            Span::raw("— Open Source Digital Signature Platform"),
        ]),
        Line::from(Span::styled(
            "Stack: Rust (Axum/Tokio) + PocketBase + TypeScript (React 18 / ShadCN UI)",
            Style::default().fg(Color::Gray),
        )),
    ])
    .alignment(Alignment::Center)
    .block(header_block);
    f.render_widget(header_text, chunks[0]);

    // Navigation Tabs Bar
    let tab_titles = vec![
        Line::from(" [1] Setup & Database Wizard "),
        Line::from(" [2] User Password Reset "),
        Line::from(" [3] Service Status & Logs "),
    ];
    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(brand_navy))
                .title(" Navigation (Press 1, 2, or 3 to switch tabs) "),
        )
        .select(app.active_tab)
        .style(Style::default().fg(Color::Gray))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, chunks[1]);

    // Main Content Area
    match app.screen {
        Screen::Login => {
            let mut lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Administrator Authentication Required",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )),
                Line::from(
                    "You are running `signline` as a non-root user. Please sign in with your Signline account:",
                ),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        if app.login_field == 0 {
                            "▶ User Email : "
                        } else {
                            "  User Email : "
                        },
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(
                            "{}{}",
                            app.login_email,
                            if app.login_field == 0 { "█" } else { "" }
                        ),
                        Style::default().fg(Color::Cyan),
                    ),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        if app.login_field == 1 {
                            "▶ Password   : "
                        } else {
                            "  Password   : "
                        },
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(
                            "{}{}",
                            "*".repeat(app.login_password.len()),
                            if app.login_field == 1 { "█" } else { "" }
                        ),
                        Style::default().fg(Color::Cyan),
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Press [Tab/↑/↓] to switch fields, [Enter] to Sign In, or [Esc] to exit.",
                    Style::default().fg(Color::Gray),
                )),
            ];
            if !app.login_error.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    &app.login_error,
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )));
            }
            let login_box = Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(brand_blue))
                        .title(" Signline CLI Login "),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(login_box, chunks[2]);
        }
        Screen::Welcome => {
            let welcome = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Welcome to the Signline v0.1.0b — Open Source Digital Signature Platform Installer!",
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("This interactive Ratatui wizard will guide you through:"),
                Line::from("  • Selecting your Database Engine (PocketBase, SQLite, PostgreSQL, or MySQL) & data path"),
                Line::from("  • Selecting your Document Storage backend (Local Disk, AWS S3, GCS, or Azure Blob)"),
                Line::from("  • Configuring Email Delivery (Gmail OAuth 2.0 or SMTP Relay)"),
                Line::from("  • Setting your Public Domain, HTTP Ports, and Initial Administrator Account"),
                Line::from("  • Enabling & starting the systemd service (signline.service)"),
                Line::from(""),
                Line::from(Span::styled(
                    "Press [ENTER] to begin setup, or [Q] to exit.",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )),
            ])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(brand_navy))
                    .title(" Welcome "),
            )
            .wrap(Wrap { trim: true });
            f.render_widget(welcome, chunks[2]);
        }
        Screen::SetupWizard => {
            let masked_pw = mask_secret(&app.config.admin_password);
            let db_choice = format!("◀ {} ▶", DB_ENGINES[app.config.db_engine_idx]);
            let storage_choice = format!("◀ {} ▶", STORAGE_PROVIDERS[app.config.storage_idx]);
            let email_choice = format!("◀ {} ▶", EMAIL_TRANSPORTS[app.config.email_transport_idx]);
            let systemd_val = if app.config.enable_systemd {
                "[x] Enable & Start Service"
            } else {
                "[ ] Do Not Start Service"
            };

            let fields: Vec<(&str, String)> = vec![
                ("1. Database Engine", db_choice),
                ("2. Database Path / URI", app.config.db_connection.clone()),
                ("3. PocketBase Engine Port", app.config.pb_port.clone()),
                ("4. Document Storage", storage_choice),
                ("5. Storage Path / Bucket", app.config.storage_path.clone()),
                ("6. Email Transport", email_choice),
                ("7. HTTP Bind Host", app.config.host.clone()),
                ("8. HTTP Listen Port", app.config.port.clone()),
                ("9. Public URL / Domain", app.config.public_url.clone()),
                ("10. Organization Name", app.config.org_name.clone()),
                ("11. Admin User Email", app.config.admin_email.clone()),
                ("12. Admin User Password", masked_pw),
                ("13. Systemd Service", systemd_val.to_string()),
            ];

            let items: Vec<ListItem> = fields
                .iter()
                .enumerate()
                .map(|(idx, (label, val))| {
                    let is_selected = idx == app.selected_field;
                    let prefix = if is_selected { "▶ " } else { "  " };
                    let style = if is_selected {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    let hint = if is_selected && app.editing {
                        " [TYPING - Press Enter when done]"
                    } else if is_selected && app.is_choice_field(idx) {
                        " [Use ←/→ or Space to change]"
                    } else if is_selected {
                        " [Press Enter to edit]"
                    } else {
                        ""
                    };
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("{}{:<26}: ", prefix, label), style),
                        Span::styled(
                            format!("{}{}", val, hint),
                            if is_selected {
                                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(Color::Gray)
                            },
                        ),
                    ]))
                })
                .collect();

            let list = List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(brand_blue))
                    .title(
                        " Setup & Database Wizard (↑/↓ Select, ←/→ Cycle Option, [Enter] Edit, [I] Apply & Restart, [Q] Quit) ",
                    ),
            );
            f.render_widget(list, chunks[2]);
        }
        Screen::Installing | Screen::Complete => {
            let inner = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Constraint::Length(3), Constraint::Min(6)])
                .split(chunks[2]);

            let gauge = Gauge::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Configuration & Service Activation Progress "),
                )
                .gauge_style(Style::default().fg(brand_blue))
                .percent(app.progress);
            f.render_widget(gauge, inner[0]);

            let mut lines: Vec<Line> = app
                .install_logs
                .iter()
                .map(|l| Line::from(format!("✓ {}", l)))
                .collect();
            if app.screen == Screen::Complete {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    format!(
                        "Configuration saved to {} and signline.service restarted!",
                        app.env_output_path
                    ),
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(Span::styled(
                    "Press [M] to return to Management TUI, or [ENTER] / [Q] to exit.",
                    Style::default().fg(Color::Yellow),
                )));
            }

            let log_box = Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(brand_navy))
                        .title(" Execution Log "),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(Clear, inner[1]);
            f.render_widget(log_box, inner[1]);
        }
        Screen::PasswordReset => {
            let inner = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                .split(chunks[2]);

            let user_items: Vec<ListItem> = if app.users.is_empty() {
                vec![ListItem::new(Line::from(
                    "  (No existing users found — enter email on the right to create admin)",
                ))]
            } else {
                app.users
                    .iter()
                    .enumerate()
                    .map(|(idx, u)| {
                        let sel = idx == app.selected_user_idx;
                        let marker = if sel { "▶ " } else { "  " };
                        ListItem::new(Line::from(vec![
                            Span::styled(
                                format!("{}{} ", marker, u.email),
                                if sel {
                                    Style::default()
                                        .fg(Color::Yellow)
                                        .add_modifier(Modifier::BOLD)
                                } else {
                                    Style::default().fg(Color::White)
                                },
                            ),
                            Span::styled(
                                format!("[{}]", u.role),
                                Style::default().fg(Color::Cyan),
                            ),
                        ]))
                    })
                    .collect()
            };

            let user_list = List::new(user_items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(brand_navy))
                    .title(" Registered Users (←/→ to Select User) "),
            );
            f.render_widget(user_list, inner[0]);

            let pw_masked = mask_secret(&app.reset_password);
            let confirm_masked = mask_secret(&app.reset_confirm);
            let form_fields = [
                ("Target User Email", app.reset_email.as_str()),
                ("New Password", pw_masked.as_str()),
                ("Confirm Password", confirm_masked.as_str()),
                ("Action", "[ Reset / Provision User Password Now ]"),
            ];

            let mut right_lines = vec![Line::from("")];
            for (idx, (label, val)) in form_fields.iter().enumerate() {
                let is_sel = idx == app.reset_field;
                let prefix = if is_sel { "▶ " } else { "  " };
                let edit_tag = if is_sel && app.reset_editing {
                    " [TYPING - Enter when done]"
                } else {
                    ""
                };
                right_lines.push(Line::from(vec![
                    Span::styled(
                        format!("{}{:<20}: ", prefix, label),
                        if is_sel {
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
                    Span::styled(
                        format!("{}{}", val, edit_tag),
                        if is_sel {
                            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::Gray)
                        },
                    ),
                ]));
                right_lines.push(Line::from(""));
            }

            right_lines.push(Line::from(Span::styled(
                "Tip: Select fields with ↑/↓, press [Enter] to type, or press [R] to Reset Password.",
                Style::default().fg(Color::Gray),
            )));

            if !app.reset_status_msg.is_empty() {
                right_lines.push(Line::from(""));
                right_lines.push(Line::from(Span::styled(
                    &app.reset_status_msg,
                    Style::default()
                        .fg(if app.reset_status_ok {
                            Color::Green
                        } else {
                            Color::Red
                        })
                        .add_modifier(Modifier::BOLD),
                )));
            }

            let reset_panel = Paragraph::new(right_lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(brand_blue))
                        .title(" Reset User Password "),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(reset_panel, inner[1]);
        }
        Screen::ServiceStatus => {
            let mut lines: Vec<Line> = app
                .service_status_lines
                .iter()
                .map(|l| Line::from(l.as_str()))
                .collect();
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Keys: [S] Restart Service   [T] Stop Service   [R] Refresh Status   [Q] Quit",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )));
            let status_box = Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(brand_blue))
                        .title(" Signline Service & PocketBase Status "),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(status_box, chunks[2]);
        }
    }

    // Mandatory AGPLv3 Section 7(b) Upstream Attribution Footer
    let footer = Paragraph::new(Line::from(vec![
        Span::raw("Powered by "),
        Span::styled(
            "Signline",
            Style::default().fg(brand_blue).add_modifier(Modifier::BOLD),
        ),
        Span::raw(", based on "),
        Span::styled(
            "Signline (https://github.com/signline/signline)",
            Style::default().fg(Color::Cyan),
        ),
        Span::raw(" — AGPLv3 License"),
    ]))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[3]);
}
