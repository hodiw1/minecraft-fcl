//! Соціальна система Terrapolis на Supabase: єдиний акаунт `@username`, друзі,
//! Rich Presence у реальному часі та запити доступу до спільних збірок.
//!
//! Усі запити неблокуючі (`reqwest`) і з коротким таймаутом. Мережеві збої обробляються
//! м'яко: лаунчер лишається повністю офлайн-працездатним, а хмара «наздоганяє», щойно
//! з'явиться інтернет (heartbeat кожні [`PRESENCE_HEARTBEAT_SECS`] секунд).
//!
//! Безпека: у базу ходимо лише з publishable-ключем; таблиці закриті RLS, а кожна RPC
//! (`docs/supabase_schema.sql`) перевіряє пару `(account_id, account_token)` — сервер знає
//! лише SHA-256 токена.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::{Client, Method, RequestBuilder};
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Notify;

use crate::commands::{load_ctx, running_error, update_config, ProfilesView, MAX_PROFILES};
use crate::config::{
    self, new_account_token, new_id, suggested_username, token_hash, validate_nickname,
    validate_username, Config, Profile, THEMES,
};
use crate::constants::{PRESENCE_HEARTBEAT_SECS, SHARED_PACKS_BUCKET, SUPABASE_ANON_KEY, SUPABASE_URL};
use crate::custom::{is_uuid, CustomSpec};
use crate::error::{fail, AppError, Result};
use crate::i18n::tr;
use crate::paths::Paths;
use crate::state::AppState;
use crate::t;

const RPC_TIMEOUT: Duration = Duration::from_secs(10);
const UPLOAD_TIMEOUT: Duration = Duration::from_secs(180);
/// Скільки чекати сервер під час закриття лаунчера (статус «офлайн»).
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);

// ───────────────────────────── помилки ─────────────────────────────

/// Помилка хмари: мережа (лаунчер працює далі офлайн) або відповідь сервера з кодом.
#[derive(Debug)]
pub enum CloudError {
    Network,
    Api(String),
}

impl CloudError {
    pub fn is_network(&self) -> bool {
        matches!(self, CloudError::Network)
    }

    pub fn code(&self) -> &str {
        match self {
            CloudError::Network => "network",
            CloudError::Api(code) => code,
        }
    }
}

/// Людський текст для коду помилки з RPC (`raise exception '<code>'`).
fn human_code(code: &str) -> String {
    let text = match code {
        "username_taken" => tr(
            "Цей @username уже зайнятий — спробуй інший",
            "This @username is already taken — try another one",
        ),
        "invalid_username" => tr(
            "@username: 3–20 символів — латиниця, цифри та _",
            "@username: 3–20 characters — Latin letters, digits and _",
        ),
        "auth_failed" => tr(
            "Хмара не впізнала акаунт — лаунчер спробує зареєструвати його повторно",
            "The cloud didn't recognise the account — the launcher will register it again",
        ),
        "user_not_found" => tr(
            "Гравця з таким @username не знайдено",
            "No player with this @username was found",
        ),
        "cannot_add_self" => tr("Себе додати в друзі не вийде", "You can't add yourself as a friend"),
        "too_many" => tr("Забагато друзів і заявок", "Too many friends and requests"),
        "request_not_found" => tr("Заявку вже оброблено", "This request has already been handled"),
        "pack_not_found" => tr(
            "Збірку не знайдено — можливо, друг зробив її приватною",
            "Modpack not found — your friend may have made it private",
        ),
        "not_friends" => tr(
            "Спершу додайте одне одного в друзі",
            "Add each other as friends first",
        ),
        "access_denied" => tr(
            "Доступу ще немає — дочекайся схвалення друга",
            "No access yet — wait for your friend to approve",
        ),
        "pack_too_large" => tr(
            "Архів збірки завеликий (понад 45 МБ)",
            "The modpack archive is too large (over 45 MB)",
        ),
        "too_many_packs" => tr(
            "Поділитися можна щонайбільше 30 збірками",
            "You can share at most 30 modpacks",
        ),
        "schema_missing" => tr(
            "Хмару Terrapolis ще не налаштовано: виконай docs/supabase_schema.sql у Supabase SQL Editor",
            "The Terrapolis cloud isn't set up yet: run docs/supabase_schema.sql in the Supabase SQL Editor",
        ),
        "storage_denied" => tr(
            "Сховище відхилило архів збірки (перевір політики бакета shared-packs)",
            "Storage rejected the modpack archive (check the shared-packs bucket policies)",
        ),
        "request_cooldown" => tr(
            "Друг щойно відхилив запит — спробуй знову за 10 хвилин",
            "Your friend just denied the request — try again in 10 minutes",
        ),
        "invalid_project_code" | "invalid_instance" | "invalid_pack" => tr(
            "Хмара відхилила некоректні дані збірки",
            "The cloud rejected invalid modpack data",
        ),
        "unauthorized" => tr(
            "Хмара відхилила ключ лаунчера",
            "The cloud rejected the launcher key",
        ),
        _ => return t!("Помилка хмари: {}", "Cloud error: {}", code),
    };
    text.to_string()
}

impl std::fmt::Display for CloudError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CloudError::Network => f.write_str(tr(
                "Немає зв'язку з хмарою Terrapolis — перевір інтернет. Гра й власні збірки працюють і без неї.",
                "Can't reach the Terrapolis cloud — check your internet. The game and custom modpacks work without it.",
            )),
            CloudError::Api(code) => f.write_str(&human_code(code)),
        }
    }
}

pub type CloudResult<T> = std::result::Result<T, CloudError>;

// ───────────────────────────── HTTP ─────────────────────────────

fn request(client: &Client, method: Method, url: &str) -> RequestBuilder {
    client
        .request(method, url)
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {SUPABASE_ANON_KEY}"))
}

/// Код помилки з тіла відповіді PostgREST / Storage: `{"code": "...", "message": "..."}`.
fn error_code(status: u16, body: &str) -> CloudError {
    let parsed: Option<Value> = serde_json::from_str(body).ok();
    let field = |name: &str| {
        parsed
            .as_ref()
            .and_then(|v| v.get(name))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let code = field("code");
    let message = field("message");
    // Gateway / база недоступні — для лаунчера це те саме, що немає мережі.
    if status >= 500 && code != "P0001" && !code.starts_with("PGRST") {
        return CloudError::Network;
    }
    if code == "PGRST202" || code == "42883" || code == "PGRST205" {
        return CloudError::Api("schema_missing".into());
    }
    if status == 401 || (status == 403 && code.is_empty()) {
        return CloudError::Api("unauthorized".into());
    }
    let message = message.trim();
    if !message.is_empty() && message.len() <= 40 && message.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
        return CloudError::Api(message.to_string());
    }
    CloudError::Api(if message.is_empty() {
        format!("http_{status}")
    } else {
        message.chars().take(120).collect()
    })
}

/// Виклик RPC-функції (`POST /rest/v1/rpc/<name>`). Порожня відповідь (void) → `Value::Null`.
pub async fn rpc(client: &Client, name: &str, body: Value) -> CloudResult<Value> {
    let url = format!("{SUPABASE_URL}/rest/v1/rpc/{name}");
    let response = request(client, Method::POST, &url)
        .header("Content-Type", "application/json")
        .timeout(RPC_TIMEOUT)
        .json(&body)
        .send()
        .await
        .map_err(|_| CloudError::Network)?;
    let status = response.status().as_u16();
    let text = response
        .text()
        .await
        .map_err(|_| CloudError::Network)?;
    if !(200..300).contains(&status) {
        return Err(error_code(status, &text));
    }
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&text).map_err(|_| CloudError::Api("bad_response".into()))
}

/// RPC від імені акаунта: до тіла додаються `p_user` і `p_token`.
pub async fn rpc_as(client: &Client, auth: &(String, String), name: &str, mut body: Value) -> CloudResult<Value> {
    if let Value::Object(map) = &mut body {
        map.insert("p_user".into(), Value::String(auth.0.clone()));
        map.insert("p_token".into(), Value::String(auth.1.clone()));
    }
    rpc(client, name, body).await
}

/// Шлях об'єкта в бакеті: `<uuid>/<uuid>.zip` — лише такі шляхи потрапляють в URL.
pub fn is_object_path(path: &str) -> bool {
    match path.split_once('/') {
        Some((owner, file)) => is_uuid(owner) && file.strip_suffix(".zip").is_some_and(is_uuid),
        None => false,
    }
}

pub fn public_object_url(path: &str) -> String {
    format!("{SUPABASE_URL}/storage/v1/object/public/{SHARED_PACKS_BUCKET}/{path}")
}

/// Завантажує архів у Storage (перезапис дозволено квитком із `publish_pack`).
pub async fn upload_object(client: &Client, path: &str, bytes: Vec<u8>) -> CloudResult<()> {
    if !is_object_path(path) {
        return Err(CloudError::Api("bad_path".into()));
    }
    let url = format!("{SUPABASE_URL}/storage/v1/object/{SHARED_PACKS_BUCKET}/{path}");
    let response = request(client, Method::POST, &url)
        .header("Content-Type", "application/zip")
        .header("x-upsert", "true")
        .header("cache-control", "max-age=60")
        .timeout(UPLOAD_TIMEOUT)
        .body(bytes)
        .send()
        .await
        .map_err(|_| CloudError::Network)?;
    let status = response.status().as_u16();
    if (200..300).contains(&status) {
        return Ok(());
    }
    let text = response.text().await.unwrap_or_default();
    Err(match error_code(status, &text) {
        CloudError::Api(code) if status == 400 || status == 403 => {
            if code.contains("security policy") || code.contains("row-level") || code == "unauthorized" {
                CloudError::Api("storage_denied".into())
            } else if code.contains("exceeded") || code.contains("too large") || status == 413 {
                CloudError::Api("pack_too_large".into())
            } else {
                CloudError::Api(code)
            }
        }
        other => other,
    })
}

/// Видаляє архів зі Storage (best effort — квиток видає `unpublish_pack` / `publish_pack`).
pub async fn delete_object(client: &Client, path: &str) {
    if !is_object_path(path) {
        return;
    }
    let url = format!("{SUPABASE_URL}/storage/v1/object/{SHARED_PACKS_BUCKET}/{path}");
    let _ = request(client, Method::DELETE, &url)
        .timeout(RPC_TIMEOUT)
        .send()
        .await;
}

// ───────────────────────────── стан ─────────────────────────────

/// Поточна ігрова сесія для Rich Presence.
#[derive(Clone, Debug, Default, Serialize)]
pub struct PlaySession {
    pub instance_id: String,
    pub pack_name: String,
    /// project | custom_private | custom_public | custom_request
    pub pack_type: String,
    pub project_code: String,
    pub project_name: String,
    pub shared_pack_id: String,
    pub character: String,
    /// Unix-час старту (секунди).
    pub started_at: u64,
}

impl PlaySession {
    pub fn for_custom(instance_id: &str, spec: &CustomSpec, character: &str) -> Self {
        let pack_type = spec.presence_type().to_string();
        let private = pack_type == "custom_private";
        Self {
            instance_id: instance_id.to_string(),
            // Назву приватної збірки друзі не бачать: лише «Грає у власну збірку».
            pack_name: if private { String::new() } else { spec.name.clone() },
            pack_type,
            shared_pack_id: if private { String::new() } else { spec.shared_id.clone() },
            character: character.to_string(),
            started_at: now_secs(),
            ..Self::default()
        }
    }

    pub fn for_project(instance_id: &str, pack_name: &str, code: &str, project_name: &str, character: &str) -> Self {
        // Сервер приймає лише безпечний код проєкту (його відкривають друзі) — інакше показуємо
        // гру без кнопки «Увійти в проєкт», але присутність усе одно працює.
        let safe = !code.is_empty()
            && code.len() <= 120
            && code.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | '-'));
        if !safe {
            return Self {
                instance_id: instance_id.to_string(),
                pack_type: "custom_private".into(),
                character: character.to_string(),
                started_at: now_secs(),
                ..Self::default()
            };
        }
        Self {
            instance_id: instance_id.to_string(),
            pack_name: pack_name.to_string(),
            pack_type: "project".into(),
            project_code: code.to_string(),
            project_name: project_name.to_string(),
            character: character.to_string(),
            started_at: now_secs(),
            ..Self::default()
        }
    }
}

pub struct SocialState {
    session: Mutex<Option<PlaySession>>,
    /// Позачерговий heartbeat (старт/кінець гри, вхід в акаунт).
    wake: Notify,
    /// 0 — невідомо, 1 — хмара доступна, 2 — офлайн (подія `cloud-status` лише при зміні).
    cloud: AtomicU8,
    /// Відкладені публікації спільних збірок уже запущено в цьому сеансі.
    republished: AtomicBool,
    /// Публікації спільних збірок — по одній за раз.
    pub share_lock: tokio::sync::Mutex<()>,
    /// Один heartbeat за раз; вихід з акаунта чекає на запит, що вже летить, — інакше
    /// запізнілий «online» зі старим токеном перекрив би щойно надісланий «offline».
    tick: tokio::sync::Mutex<()>,
}

impl SocialState {
    pub fn new() -> Self {
        Self {
            session: Mutex::new(None),
            wake: Notify::new(),
            cloud: AtomicU8::new(0),
            republished: AtomicBool::new(false),
            share_lock: tokio::sync::Mutex::new(()),
            tick: tokio::sync::Mutex::new(()),
        }
    }

    pub fn wake(&self) {
        self.wake.notify_one();
    }

    fn session(&self) -> Option<PlaySession> {
        self.session.lock().ok().and_then(|s| s.clone())
    }

    fn set_session(&self, session: Option<PlaySession>) {
        if let Ok(mut slot) = self.session.lock() {
            *slot = session;
        }
        self.wake();
    }
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Unix-секунди → `2026-11-28T10:15:00Z` (без зовнішніх крейтів).
pub fn iso_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

// ───────────────────────────── Rich Presence ─────────────────────────────

/// Гра запустилася (викликає `launch::spawn` після успішного старту процесу). Час старту —
/// саме зараз, а не момент натискання «Грати»: встановлення збірки в таймер гри не входить.
pub fn game_started(app: &AppHandle, mut session: PlaySession) {
    session.started_at = now_secs();
    app.state::<AppState>().social.set_session(Some(session));
}

/// Процес гри завершився — повертаємо статус «В мережі».
pub fn game_finished(app: &AppHandle) {
    app.state::<AppState>().social.set_session(None);
}

fn character_of(config: &Config) -> String {
    config
        .active_profile()
        .map(|p| p.nickname.clone())
        .unwrap_or_default()
}

async fn send_presence(
    client: &Client,
    auth: &(String, String),
    status: &str,
    character: &str,
    session: Option<&PlaySession>,
) -> CloudResult<Value> {
    let body = match session {
        Some(s) if status == "playing" => json!({
            "p_status": "playing",
            "p_character": s.character,
            "p_pack_name": s.pack_name,
            "p_pack_type": s.pack_type,
            "p_instance_id": s.instance_id,
            "p_project_code": s.project_code,
            "p_project_name": s.project_name,
            "p_shared_pack_id": if is_uuid(&s.shared_pack_id) { Value::String(s.shared_pack_id.clone()) } else { Value::Null },
            "p_started_at": iso_utc(s.started_at),
        }),
        _ => json!({ "p_status": status, "p_character": character }),
    };
    rpc_as(client, auth, "update_presence", body).await
}

fn set_cloud(app: &AppHandle, online: bool) {
    let state = app.state::<AppState>();
    let value = if online { 1 } else { 2 };
    if state.social.cloud.swap(value, Ordering::SeqCst) != value {
        let _ = app.emit("cloud-status", json!({ "online": online }));
    }
}

/// Реєструє офлайн-створений акаунт, щойно з'явилася мережа. `None` — хмара недоступна
/// або ім'я виявилося зайнятим (тоді інтерфейс попросить обрати інше).
async fn ensure_registered(app: &AppHandle, state: &AppState, config: &Config) -> Option<Config> {
    if config.username.is_empty() || config.account_token.is_empty() || config.account_conflict {
        return None;
    }
    let result = rpc(
        &state.client,
        "register_account",
        json!({ "p_username": config.username, "p_token_hash": token_hash(&config.account_token) }),
    )
    .await;
    match result {
        Ok(value) => {
            let id = value.get("id").and_then(Value::as_str).unwrap_or("").to_string();
            if !is_uuid(&id) {
                return None;
            }
            let name = value
                .get("username")
                .and_then(Value::as_str)
                .filter(|n| validate_username(n).is_ok())
                .unwrap_or(&config.username)
                .to_string();
            let (token, username) = (config.account_token.clone(), config.username.clone());
            let updated = update_config(app, state, move |c| {
                // Поки летів запит, гравець міг вийти з акаунта чи обрати інше ім'я — тоді не чіпаємо.
                if c.account_token != token || !c.username.eq_ignore_ascii_case(&username) {
                    return Ok(None);
                }
                c.account_id = id;
                c.username = name;
                c.account_synced = true;
                c.account_conflict = false;
                Ok(Some(c.clone()))
            })
            .await
            .ok()
            .flatten()?;
            let _ = app.emit("account-changed", account_view(&updated));
            Some(updated)
        }
        Err(CloudError::Api(code)) if code == "username_taken" => {
            let (token, username) = (config.account_token.clone(), config.username.clone());
            if let Ok(Some(updated)) = update_config(app, state, move |c| {
                if c.account_token != token || !c.username.eq_ignore_ascii_case(&username) || c.account_synced {
                    return Ok(None);
                }
                c.account_conflict = true;
                Ok(Some(c.clone()))
            })
            .await
            {
                let _ = app.emit("account-changed", account_view(&updated));
            }
            None
        }
        Err(error) => {
            set_cloud(app, !error.is_network());
            None
        }
    }
}

async fn heartbeat(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.is_shutting_down() {
        return;
    }
    let _tick = state.social.tick.lock().await;
    let Ok(root) = Paths::app_root(app) else {
        return;
    };
    let mut config = config::load(&root).await;
    if !config.onboarded || config.username.is_empty() || config.account_conflict {
        return;
    }
    if config.online_account().is_none() {
        match ensure_registered(app, &state, &config).await {
            Some(updated) => config = updated,
            None => return,
        }
    }
    let Some(auth) = config.online_account() else {
        return;
    };
    let session = state.social.session();
    let status = if session.is_some() { "playing" } else { "online" };
    match send_presence(&state.client, &auth, status, &character_of(&config), session.as_ref()).await {
        Ok(_) => {
            set_cloud(app, true);
            // Збірки, які стали спільними офлайн, публікуються, щойно з'явилася хмара.
            if !state.social.republished.swap(true, Ordering::SeqCst) {
                crate::share::republish_all_in_background(app.clone());
            }
        }
        Err(CloudError::Api(code)) if code == "auth_failed" => {
            // База могла бути скинута: реєструємо той самий @username повторно (лише цей акаунт).
            let token = auth.1.clone();
            let _ = update_config(app, &state, move |c| {
                if c.account_token == token {
                    c.account_synced = false;
                }
                Ok(())
            })
            .await;
            state.social.wake();
        }
        Err(error) => set_cloud(app, !error.is_network()),
    }
}

/// Фоновий heartbeat присутності: кожні 45 с або одразу після старту/кінця гри.
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            heartbeat(&app).await;
            let state = app.state::<AppState>();
            let _ = tokio::time::timeout(
                Duration::from_secs(PRESENCE_HEARTBEAT_SECS),
                state.social.wake.notified(),
            )
            .await;
        }
    });
}

/// Закриття лаунчера: статус «Офлайн» (не довше за 3 секунди).
pub fn shutdown(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::block_on(async move {
        let _ = tokio::time::timeout(SHUTDOWN_TIMEOUT, async {
            let Ok(root) = Paths::app_root(&app) else {
                return;
            };
            let state = app.state::<AppState>();
            // Запізнілий heartbeat не має перекрити «offline».
            let _tick = state.social.tick.lock().await;
            let config = config::load(&root).await;
            if let Some(auth) = config.online_account() {
                let _ = send_presence(&state.client, &auth, "offline", &character_of(&config), None).await;
            }
        })
        .await;
    });
}

// ───────────────────────────── акаунт ─────────────────────────────

#[derive(Serialize, Clone)]
pub struct AccountView {
    pub account_id: String,
    pub username: String,
    /// @username підтверджено сервером.
    pub synced: bool,
    /// Офлайн-створене ім'я зайняте — треба обрати інше.
    pub conflict: bool,
    /// Потрібне вікно вибору @username (оновлення з v0.4.0 або конфлікт).
    pub needs_username: bool,
    /// Підказка для поля @username (основний нік).
    pub suggested: String,
}

pub fn account_view(config: &Config) -> AccountView {
    AccountView {
        account_id: config.account_id.clone(),
        username: config.username.clone(),
        synced: config.account_synced,
        conflict: config.account_conflict,
        needs_username: config.needs_username(),
        suggested: suggested_username(config),
    }
}

#[derive(Serialize)]
pub struct ClaimOutcome {
    pub account: AccountView,
    pub profiles: ProfilesView,
    /// Хмара недоступна — акаунт створено локально, реєстрація відкладена.
    pub offline: bool,
    /// Повторний вхід: повернуто ніки акаунта, з якого раніше вийшли на цьому ПК.
    pub restored: bool,
    pub theme: String,
}

#[tauri::command]
pub async fn account_info(app: AppHandle) -> Result<AccountView> {
    let (config, _) = load_ctx(&app).await?;
    Ok(account_view(&config))
}

/// Спільне для онбордингу та вікна оновлення: реєстрація `@username` у Supabase
/// (з відкладеною синхронізацією без інтернету) і збереження акаунта в config.json.
async fn claim(
    app: &AppHandle,
    state: &AppState,
    username: String,
    extra: Option<(String, String)>,
) -> Result<ClaimOutcome> {
    let (config, _) = load_ctx(app).await?;
    let username = username.trim().trim_start_matches('@').to_string();
    validate_username(&username)?;
    if let Some((nickname, theme)) = &extra {
        validate_nickname(nickname)?;
        if !THEMES.contains(&theme.as_str()) {
            return fail("unknown theme");
        }
    }
    if config.account_synced && !config.username.eq_ignore_ascii_case(&username) {
        return fail(tr(
            "Акаунт уже створено — спершу вийди з нього",
            "An account already exists — sign out first",
        ));
    }

    let remembered = config
        .remembered
        .iter()
        .find(|r| r.username.eq_ignore_ascii_case(&username))
        .cloned();
    let token = match &remembered {
        Some(r) => r.account_token.clone(),
        None if !config.account_token.is_empty() => config.account_token.clone(),
        None => new_account_token()?,
    };

    let result = rpc(
        &state.client,
        "register_account",
        json!({ "p_username": username, "p_token_hash": token_hash(&token) }),
    )
    .await;
    let (account_id, name, synced, offline) = match result {
        Ok(value) => {
            let id = value.get("id").and_then(Value::as_str).unwrap_or("").to_string();
            if !is_uuid(&id) {
                return fail(human_code("bad_response"));
            }
            let name = value
                .get("username")
                .and_then(Value::as_str)
                .filter(|n| validate_username(n).is_ok())
                .unwrap_or(&username)
                .to_string();
            set_cloud(app, true);
            (id, name, true, false)
        }
        Err(error) if error.is_network() => {
            set_cloud(app, false);
            // Повторний вхід офлайн: id акаунта вже підтверджував сервер.
            match remembered.as_ref().filter(|r| is_uuid(&r.account_id)) {
                Some(r) => (r.account_id.clone(), r.username.clone(), true, true),
                None => (String::new(), username.clone(), false, true),
            }
        }
        Err(error) => return Err(AppError(error.to_string())),
    };

    let (profiles, restored, theme) = update_config(app, state, move |config| {
        let restored = match config.take_remembered(&name) {
            Some(back) => {
                if config.profiles.is_empty() {
                    config.profiles = back.characters;
                    config.active_profile = back.active_character;
                }
                true
            }
            None => false,
        };
        config.username = name;
        config.account_token = token;
        config.account_id = account_id;
        config.account_synced = synced;
        config.account_conflict = false;
        if let Some((nickname, theme)) = extra {
            config.theme = theme;
            if let Some(existing) = config
                .profiles
                .iter()
                .find(|p| p.nickname.eq_ignore_ascii_case(&nickname))
            {
                config.active_profile = existing.id.clone();
            } else {
                if config.profiles.len() >= MAX_PROFILES {
                    return fail(tr("Забагато ніків (максимум 20)", "Too many nicknames (max 20)"));
                }
                let profile = Profile {
                    id: new_id(),
                    nickname,
                    has_skin: false,
                };
                config.active_profile = profile.id.clone();
                config.profiles.push(profile);
            }
        }
        if !config.profiles.iter().any(|p| p.id == config.active_profile) {
            config.active_profile = config.profiles.first().map(|p| p.id.clone()).unwrap_or_default();
        }
        Ok((ProfilesView::of(config), restored, config.theme.clone()))
    })
    .await?;

    let (config, _) = load_ctx(app).await?;
    state.social.wake();
    Ok(ClaimOutcome {
        account: account_view(&config),
        profiles,
        offline,
        restored,
        theme,
    })
}

/// Крок 1 онбордингу: `@username` акаунта, перший ігровий нік і палітра.
#[tauri::command]
pub async fn onboarding_account(
    app: AppHandle,
    state: State<'_, AppState>,
    username: String,
    nickname: String,
    theme: String,
) -> Result<ClaimOutcome> {
    let nickname = nickname.trim().to_string();
    claim(&app, &state, username, Some((nickname, theme))).await
}

/// Вікно оновлення до v0.4.1 (або конфлікт імені): лише `@username`, усе інше вже є.
#[tauri::command]
pub async fn claim_username(
    app: AppHandle,
    state: State<'_, AppState>,
    username: String,
) -> Result<ClaimOutcome> {
    let outcome = claim(&app, &state, username, None).await?;
    // Гравець уже був у лаунчері — майстер не потрібен.
    update_config(&app, &state, |config| {
        if !config.profiles.is_empty() {
            config.onboarded = true;
        }
        Ok(())
    })
    .await?;
    Ok(outcome)
}

/// «Вийти з акаунта»: статус `offline` на сервері, очищення акаунта й повернення до онбордингу.
#[tauri::command]
pub async fn logout_account(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    if state.running_game().is_some() {
        return Err(running_error());
    }
    // Чекаємо heartbeat, що вже летить, і не даємо почати новий до кінця виходу.
    let _tick = state.social.tick.lock().await;
    let (config, _) = load_ctx(&app).await?;
    if let Some(auth) = config.online_account() {
        // Без інтернету вихід усе одно відбувається — сервер сам погасить статус за 150 с.
        let _ = rpc_as(&state.client, &auth, "sign_out", json!({})).await;
    }
    if let Ok(mut slot) = state.social.session.lock() {
        *slot = None;
    }
    state.social.cloud.store(0, Ordering::SeqCst);
    state.social.republished.store(false, Ordering::SeqCst);
    update_config(&app, &state, |config| {
        config.sign_out();
        Ok(())
    })
    .await
}

// ───────────────────────────── друзі ─────────────────────────────

fn require_auth(config: &Config) -> Result<(String, String)> {
    config.online_account().ok_or_else(|| {
        AppError(
            tr(
                "Акаунт ще не синхронізовано з хмарою — перевір інтернет",
                "The account isn't synced with the cloud yet — check your internet",
            )
            .to_string(),
        )
    })
}

fn check_uuid(value: &str) -> Result<()> {
    if is_uuid(value) {
        Ok(())
    } else {
        fail("invalid id")
    }
}

async fn call(app: &AppHandle, state: &AppState, name: &str, body: Value) -> Result<Value> {
    let (config, _) = load_ctx(app).await?;
    let auth = require_auth(&config)?;
    match rpc_as(&state.client, &auth, name, body).await {
        Ok(value) => {
            set_cloud(app, true);
            Ok(value)
        }
        Err(error) => {
            if error.is_network() {
                set_cloud(app, false);
            }
            if error.code() == "auth_failed" {
                state.social.wake();
            }
            Err(AppError(error.to_string()))
        }
    }
}

#[derive(Serialize)]
pub struct SocialSnapshot {
    /// Хмара відповіла.
    pub online: bool,
    /// Акаунт ще не зареєстровано (офлайн-створення) — друзі з'являться після синхронізації.
    pub pending: bool,
    pub error: Option<String>,
    /// Відповідь `social_snapshot`: friends, incoming, outgoing, pack_requests, server_now.
    pub data: Value,
}

/// Усе для вкладки «Друзі» одним запитом. Мережеві збої не є помилкою — лише `online: false`.
#[tauri::command]
pub async fn social_snapshot(app: AppHandle, state: State<'_, AppState>) -> Result<SocialSnapshot> {
    let (config, _) = load_ctx(&app).await?;
    let Some(auth) = config.online_account() else {
        return Ok(SocialSnapshot {
            online: false,
            pending: !config.username.is_empty(),
            error: None,
            data: Value::Null,
        });
    };
    match rpc_as(&state.client, &auth, "social_snapshot", json!({})).await {
        Ok(data) => {
            set_cloud(&app, true);
            Ok(SocialSnapshot {
                online: true,
                pending: false,
                error: None,
                data,
            })
        }
        Err(error) => {
            set_cloud(&app, !error.is_network());
            if error.code() == "auth_failed" {
                state.social.wake();
            }
            Ok(SocialSnapshot {
                online: false,
                pending: false,
                error: Some(error.to_string()),
                data: Value::Null,
            })
        }
    }
}

#[tauri::command]
pub async fn search_users(app: AppHandle, state: State<'_, AppState>, query: String) -> Result<Value> {
    let query: String = query
        .trim()
        .trim_start_matches('@')
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(20)
        .collect();
    if query.len() < 2 {
        return Ok(Value::Array(Vec::new()));
    }
    call(&app, &state, "search_users", json!({ "p_query": query })).await
}

#[tauri::command]
pub async fn send_friend_request(app: AppHandle, state: State<'_, AppState>, username: String) -> Result<Value> {
    let username = username.trim().trim_start_matches('@').to_string();
    validate_username(&username)?;
    let (config, _) = load_ctx(&app).await?;
    if config.username.eq_ignore_ascii_case(&username) {
        return fail(human_code("cannot_add_self"));
    }
    call(&app, &state, "send_friend_request", json!({ "p_username": username })).await
}

#[tauri::command]
pub async fn respond_friend_request(
    app: AppHandle,
    state: State<'_, AppState>,
    request_id: String,
    accept: bool,
) -> Result<()> {
    check_uuid(&request_id)?;
    call(&app, &state, "respond_friend_request", json!({ "p_request": request_id, "p_accept": accept }))
        .await
        .map(|_| ())
}

/// Видалити друга або скасувати власну заявку.
#[tauri::command]
pub async fn remove_friend(app: AppHandle, state: State<'_, AppState>, friend_id: String) -> Result<()> {
    check_uuid(&friend_id)?;
    call(&app, &state, "remove_friend", json!({ "p_friend": friend_id }))
        .await
        .map(|_| ())
}

#[tauri::command]
pub async fn request_pack_access(app: AppHandle, state: State<'_, AppState>, pack_id: String) -> Result<Value> {
    check_uuid(&pack_id)?;
    call(&app, &state, "request_pack_access", json!({ "p_pack": pack_id })).await
}

#[tauri::command]
pub async fn respond_pack_request(
    app: AppHandle,
    state: State<'_, AppState>,
    request_id: String,
    approve: bool,
) -> Result<()> {
    check_uuid(&request_id)?;
    call(&app, &state, "respond_pack_request", json!({ "p_request": request_id, "p_approve": approve }))
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_dates_are_correct() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(iso_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(iso_utc(1_795_849_200), "2026-11-28T07:00:00Z");
    }

    #[test]
    fn object_paths_are_strict() {
        let id = "8a6e0804-2bd0-4672-b79d-d97027f9071a";
        assert!(is_object_path(&format!("{id}/{id}.zip")));
        for bad in [
            format!("{id}/../{id}.zip"),
            format!("{id}/{id}.zip?x=1"),
            format!("{id}/{id}"),
            format!("../{id}.zip"),
            format!("{id}/{id}.zip/extra"),
            String::from(""),
        ] {
            assert!(!is_object_path(&bad), "{bad}");
        }
    }

    #[test]
    fn rpc_errors_are_classified() {
        let taken = error_code(400, r#"{"code":"P0001","message":"username_taken","details":null}"#);
        assert_eq!(taken.code(), "username_taken");
        assert!(!taken.is_network());
        let missing = error_code(404, r#"{"code":"PGRST202","message":"Could not find the function"}"#);
        assert_eq!(missing.code(), "schema_missing");
        assert!(error_code(503, "upstream connect error").is_network());
        assert_eq!(error_code(401, "{}").code(), "unauthorized");
    }

    #[test]
    fn private_presence_hides_the_pack() {
        let spec = CustomSpec {
            name: "Secret base".into(),
            visibility: "private".into(),
            shared_id: "8a6e0804-2bd0-4672-b79d-d97027f9071a".into(),
            ..CustomSpec::default()
        };
        let session = PlaySession::for_custom("my-1", &spec, "Steve");
        assert_eq!(session.pack_type, "custom_private");
        assert!(session.pack_name.is_empty());
        assert!(session.shared_pack_id.is_empty());

        let spec = CustomSpec {
            visibility: "on_request".into(),
            ..spec
        };
        let session = PlaySession::for_custom("my-1", &spec, "Steve");
        assert_eq!(session.pack_type, "custom_request");
        assert_eq!(session.pack_name, "Secret base");
        assert!(!session.shared_pack_id.is_empty());
    }
}
