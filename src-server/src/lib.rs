use std::io::Write as _;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use axum::{
    body::Body,
    extract::{multipart::Multipart, Path, Query, State},
    http::{header, request::Parts as RequestParts, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    limit::RequestBodyLimitLayer,
    trace::TraceLayer,
};
use clantrail_core::{
    new_uuid, AppError, EntityType, NewBurialGroup, NewClan, NewEdge, NewGrave, NewImage,
    NewMember, ClanTrailDb,
};

pub type Result<T> = std::result::Result<T, ApiError>;

/// rusqlite::Connection 不是 Sync，用 Mutex 包装以通过 axum 的 Send+Sync 约束。
/// 所有数据库操作都是同步短调用，不跨 await 持锁，std::sync::Mutex 足够。
type SharedDb = Arc<Mutex<ClanTrailDb>>;

#[derive(Clone)]
pub struct AppState {
    pub db: SharedDb,
    /// SQLite 数据库文件路径（绝对路径）。导出/导入时需要。
    pub db_path: PathBuf,
    /// 照片本地存储目录（绝对路径）。DB 只存相对路径，便于备份迁移。
    pub upload_dir: PathBuf,
}

impl AppState {
    pub fn new(db: ClanTrailDb, db_path: PathBuf, upload_dir: PathBuf) -> Self {
        AppState {
            db: Arc::new(Mutex::new(db)),
            db_path,
            upload_dir,
        }
    }

    pub fn db(&self) -> Result<MutexGuard<'_, ClanTrailDb>> {
        self.db
            .lock()
            .map_err(|_| ApiError(AppError::General("db lock poisoned".into())))
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SearchQuery {
    keyword: String,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
struct ReminderQuery {
    days: Option<u32>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
struct PageQuery {
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
struct MemberQuery {
    #[serde(default)]
    alive: Option<bool>,
}

/// 统一 API 错误：把核心库错误映射成 HTTP 状态码
pub struct ApiError(AppError);

impl From<AppError> for ApiError {
    fn from(e: AppError) -> Self {
        ApiError(e)
    }
}

impl From<zip::result::ZipError> for ApiError {
    fn from(e: zip::result::ZipError) -> Self {
        ApiError(AppError::General(e.to_string()))
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        ApiError(AppError::Serde(e))
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        ApiError(AppError::General(e.to_string()))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match self.0 {
            AppError::NotFound(what) => (StatusCode::NOT_FOUND, format!("not found: {what}")),
            AppError::InvalidInput(m) => (StatusCode::BAD_REQUEST, m),
            // 内部错误只返回泛化消息，具体细节记日志，避免泄露内部实现。
            AppError::Database(e) => {
                tracing::error!("database error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error".into())
            }
            AppError::Serde(e) => {
                tracing::error!("serialization error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error".into())
            }
            AppError::Migration(e) => {
                tracing::error!("migration error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error".into())
            }
            AppError::General(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
        };
        (status, Json(serde_json::json!({ "error": msg }))).into_response()
    }
}

/// 构建路由（不绑定端口）。供独立二进制与 Tauri 内嵌后端共用。
pub fn build_router(state: AppState) -> Router {
    let mut router = Router::new()
        .route("/api/health", get(health))
        // Clan
        .route("/api/clans", get(list_clans).post(create_clan))
        .route(
            "/api/clans/:id",
            get(get_clan).put(update_clan).delete(delete_clan),
        )
        .route("/api/clans/:id/groups", get(list_groups))
        .route("/api/clans/:id/graves", get(list_clan_graves))
        .route("/api/clans/:id/members", get(list_clan_members))
        .route("/api/clans/:id/graph", get(clan_graph))
        // BurialGroup
        .route("/api/burial-groups", post(create_group))
        .route("/api/burial-groups/:id", delete(delete_group))
        // Grave
        .route("/api/graves", get(list_graves).post(create_grave))
        .route(
            "/api/graves/:id",
            get(get_grave).put(update_grave).delete(delete_grave),
        )
        .route("/api/graves/:id/members", get(list_members))
        .route("/api/graves/:id/images", get(list_images))
        // Member
        .route("/api/members", post(create_member))
        .route(
            "/api/members/:id",
            get(get_member_handler).put(update_member).delete(delete_member),
        )
        .route("/api/members/:id/images", get(list_member_images))
        // Edge
        .route("/api/members/:id/edges", get(list_member_edges).post(create_member_edge))
        .route("/api/edges/:id", delete(delete_edge))
        // 关系图
        .route("/api/members/:id/egograph", get(member_egograph))
        // Image
        .route("/api/images", post(create_image))
        .route("/api/images/upload", post(upload_image))
        .route("/api/images/upload64", post(upload_image_base64))
        .route("/api/images/files/*path", get(serve_image_file))
        .route("/api/images/:id", put(update_image).delete(delete_image))
        // 数据导出/导入与备份管理
        .route("/api/export", post(create_export))
        .route("/api/backups", get(list_backups))
        .route("/api/backups/:filename", get(download_backup).delete(delete_backup))
        .route("/api/import", post(import_backup))
        // 祭祀/忌日提醒
        .route("/api/reminders", get(list_reminders))
        // 搜索
        .route("/api/search", get(search));

    // 开发种子数据（仅 debug 构建注册，避免生产暴露）
    if cfg!(debug_assertions) {
        router = router.route("/api/dev/seed", post(seed_dev_data));
    } else {
        router = router.route("/api/dev/seed", post(seed_disabled));
    }

    router
        .layer(cors_layer())
        // 请求体大小上限：照片上传可能较大，放宽到 50MB；同时防止恶意超大请求。
        .layer(RequestBodyLimitLayer::new(50 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// 仅允许本地回环来源的 CORS，避免任意网页跨域读写本地数据。
/// 使用 predicate 动态匹配，覆盖：
/// - Tauri 内嵌 WebView（tauri://localhost、http://tauri.localhost、https://tauri.localhost）
/// - Web 开发服务器（localhost/127.0.0.1 任意端口）
fn cors_layer() -> CorsLayer {
    let is_local_origin = |origin: &HeaderValue, _req: &RequestParts| -> bool {
        origin.to_str().ok().is_some_and(|s| {
            s == "tauri://localhost"
                || s.starts_with("http://tauri.localhost")
                || s.starts_with("https://tauri.localhost")
                || s.starts_with("http://127.0.0.1")
                || s.starts_with("http://localhost")
        })
    };
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(is_local_origin))
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE])
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

// ---------------------------------------------------------------------------
// Clan
// ---------------------------------------------------------------------------
/// 对结果做内存分页：limit/offset 缺省时返回全部，保持向后兼容。
fn paginate<T>(items: Vec<T>, limit: Option<usize>, offset: Option<usize>) -> Vec<T> {
    let start = offset.unwrap_or(0).min(items.len());
    match limit {
        Some(n) => items.into_iter().skip(start).take(n).collect(),
        None => items.into_iter().skip(start).collect(),
    }
}

async fn list_clans(
    State(s): State<AppState>,
    Query(q): Query<PageQuery>,
) -> Result<Json<Vec<clantrail_core::Clan>>> {
    Ok(Json(paginate(s.db()?.list_clans()?, q.limit, q.offset)))
}

async fn create_clan(
    State(s): State<AppState>,
    Json(input): Json<NewClan>,
) -> Result<Json<clantrail_core::Clan>> {
    Ok(Json(s.db()?.create_clan(input)?))
}

async fn get_clan(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<clantrail_core::Clan>> {
    Ok(Json(s.db()?.get_clan(&id)?))
}

async fn update_clan(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateClanBody>,
) -> Result<Json<clantrail_core::Clan>> {
    Ok(Json(s.db()?.update_clan(&id, body.name, body.description, body.origin)?))
}

async fn delete_clan(State(s): State<AppState>, Path(id): Path<String>) -> Result<StatusCode> {
    s.db()?.delete_clan(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_groups(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<clantrail_core::BurialGroup>>> {
    Ok(Json(s.db()?.list_groups_by_clan(&id)?))
}

async fn list_clan_graves(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<clantrail_core::Grave>>> {
    Ok(Json(s.db()?.list_graves_by_clan(&id)?))
}

// ---------------------------------------------------------------------------
// BurialGroup
// ---------------------------------------------------------------------------
async fn create_group(
    State(s): State<AppState>,
    Json(input): Json<NewBurialGroup>,
) -> Result<Json<clantrail_core::BurialGroup>> {
    Ok(Json(s.db()?.create_burial_group(input)?))
}

async fn delete_group(State(s): State<AppState>, Path(id): Path<String>) -> Result<StatusCode> {
    s.db()?.delete_burial_group(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Grave
// ---------------------------------------------------------------------------
async fn list_graves(
    State(s): State<AppState>,
    Query(q): Query<PageQuery>,
) -> Result<Json<Vec<clantrail_core::Grave>>> {
    Ok(Json(paginate(s.db()?.list_graves()?, q.limit, q.offset)))
}

async fn create_grave(
    State(s): State<AppState>,
    Json(input): Json<NewGrave>,
) -> Result<Json<clantrail_core::Grave>> {
    Ok(Json(s.db()?.create_grave(input)?))
}

async fn get_grave(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<clantrail_core::Grave>> {
    Ok(Json(s.db()?.get_grave(&id)?))
}

async fn update_grave(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateGraveBody>,
) -> Result<Json<clantrail_core::Grave>> {
    Ok(Json(s.db()?.update_grave(
        &id,
        body.name,
        body.latitude,
        body.longitude,
        body.address,
        body.description,
        body.burial_group_id,
        body.clan_id,
    )?))
}

async fn delete_grave(State(s): State<AppState>, Path(id): Path<String>) -> Result<StatusCode> {
    s.db()?.delete_grave(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_members(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<clantrail_core::Member>>> {
    Ok(Json(s.db()?.list_members_by_grave(&id)?))
}

async fn get_member_handler(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<clantrail_core::Member>> {
    Ok(Json(s.db()?.get_member(&id)?))
}

async fn list_clan_members(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<MemberQuery>,
) -> Result<Json<Vec<clantrail_core::Member>>> {
    Ok(Json(s.db()?.list_members_by_clan(&id, q.alive)?))
}

async fn clan_graph(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<GraphResponse>> {
    let db = s.db()?;
    let (members, edges) = db.list_graph_by_clan(&id)?;
    Ok(Json(GraphResponse { members, edges }))
}

async fn member_egograph(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<GraphResponse>> {
    let db = s.db()?;
    let (members, edges) = db.list_egograph(&id, 2)?;
    Ok(Json(GraphResponse { members, edges }))
}

async fn list_images(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<clantrail_core::Image>>> {
    Ok(Json(s.db()?.list_images_by_entity(
        clantrail_core::EntityType::Grave,
        &id,
    )?))
}

async fn list_member_images(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<clantrail_core::Image>>> {
    Ok(Json(s.db()?.list_images_by_entity(
        clantrail_core::EntityType::Member,
        &id,
    )?))
}

// ---------------------------------------------------------------------------
// Member
// ---------------------------------------------------------------------------
async fn create_member(
    State(s): State<AppState>,
    Json(input): Json<NewMember>,
) -> Result<Json<clantrail_core::Member>> {
    Ok(Json(s.db()?.create_member(input)?))
}

async fn update_member(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateMemberBody>,
) -> Result<Json<clantrail_core::Member>> {
    Ok(Json(s.db()?.update_member(
        &id,
        body.name,
        body.title,
        body.birth_date,
        body.death_date,
        body.biography,
        body.epitaph,
        body.spouse,
        body.is_joint_burial,
        body.children,
        body.order_index,
        body.clan_id,
        body.is_alive,
    )?))
}

async fn delete_member(State(s): State<AppState>, Path(id): Path<String>) -> Result<StatusCode> {
    s.db()?.delete_member(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Edge
// ---------------------------------------------------------------------------
async fn list_member_edges(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<clantrail_core::Edge>>> {
    Ok(Json(s.db()?.list_edges_by_member(&id)?))
}

async fn create_member_edge(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<NewEdge>,
) -> Result<Json<Vec<clantrail_core::Edge>>> {
    if input.member_id != id {
        return Err(ApiError(AppError::InvalidInput(
            "member_id in path does not match body".into(),
        )));
    }
    Ok(Json(s.db()?.create_edge(input)?))
}

async fn delete_edge(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    s.db()?.delete_edge(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Image
// ---------------------------------------------------------------------------
async fn create_image(
    State(s): State<AppState>,
    Json(input): Json<NewImage>,
) -> Result<Json<clantrail_core::Image>> {
    Ok(Json(s.db()?.add_image(input)?))
}

async fn delete_image(State(s): State<AppState>, Path(id): Path<String>) -> Result<StatusCode> {
    let rel_path = {
        let db = s.db()?;
        let image = db.get_image(&id)?;
        db.delete_image(&id)?;
        image.file_path
    };
    // best-effort 删除物理文件，失败不影响接口返回
    let full = s.upload_dir.join(&rel_path);
    let _ = tokio::fs::remove_file(&full).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn update_image(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateImageBody>,
) -> Result<Json<clantrail_core::Image>> {
    Ok(Json(s.db()?.set_image_cover(&id, body.is_cover)?))
}

// ---------------------------------------------------------------------------
// 真实照片上传：multipart/form-data（file, entity_type, entity_id, caption?）
// ---------------------------------------------------------------------------
async fn upload_image(
    State(s): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<clantrail_core::Image>> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut original_name: Option<String> = None;
    let mut entity_type: Option<String> = None;
    let mut entity_id: Option<String> = None;
    let mut caption: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError(AppError::InvalidInput(e.to_string())))?
    {
        let name = field.name().map(|s| s.to_string());
        match name.as_deref() {
            Some("file") => {
                original_name = field.file_name().map(|s| s.to_string());
                file_bytes = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| ApiError(AppError::InvalidInput(e.to_string())))?
                        .to_vec(),
                );
            }
            Some("entity_type") => {
                entity_type = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| ApiError(AppError::InvalidInput(e.to_string())))?,
                );
            }
            Some("entity_id") => {
                entity_id = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| ApiError(AppError::InvalidInput(e.to_string())))?,
                );
            }
            Some("caption") => {
                caption = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| ApiError(AppError::InvalidInput(e.to_string())))?,
                )
            }
            _ => {}
        }
    }

    let file_bytes =
        file_bytes.ok_or_else(|| ApiError(AppError::InvalidInput("缺少上传文件".into())))?;
    let entity_type =
        entity_type.ok_or_else(|| ApiError(AppError::InvalidInput("缺少 entity_type".into())))?;
    let entity_id =
        entity_id.ok_or_else(|| ApiError(AppError::InvalidInput("缺少 entity_id".into())))?;

    // 文本字段长度限制，防止异常数据撑爆
    const MAX_ENTITY_ID: usize = 128;
    const MAX_CAPTION: usize = 1024;
    if entity_id.len() > MAX_ENTITY_ID {
        return Err(ApiError(AppError::InvalidInput(format!(
            "entity_id 过长（上限 {MAX_ENTITY_ID}）"
        ))));
    }
    if let Some(cap) = &caption {
        if cap.len() > MAX_CAPTION {
            return Err(ApiError(AppError::InvalidInput(format!(
                "caption 过长（上限 {MAX_CAPTION}）"
            ))));
        }
    }

    let et = EntityType::from_str(&entity_type)
        .ok_or_else(|| ApiError(AppError::InvalidInput(format!("未知 entity_type: {entity_type}"))))?;

    // 仅接受常见图片扩展名；从原始文件名取扩展名，缺失则按二进制处理
    let ext = original_name
        .as_deref()
        .and_then(|n| std::path::Path::new(n).extension())
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_else(|| "bin".into());
    if !matches!(
        ext.as_str(),
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "heif"
    ) {
        return Err(ApiError(AppError::InvalidInput(format!(
            "不支持的图片类型: .{ext}"
        ))));
    }

    // 魔数校验：以真实文件头判断图片格式，避免仅改扩展名上传非图片内容
    let kind = detect_image_kind(&file_bytes);
    if kind.is_none() {
        return Err(ApiError(AppError::InvalidInput(
            "文件内容不是受支持的图片格式".into(),
        )));
    }

    // 校验关联实体存在（持同一把锁，不跨 await）
    let db = s.db()?;
    match et {
        EntityType::Grave => {
            db.get_grave(&entity_id)?;
        }
        EntityType::Member => {
            db.get_member(&entity_id)?;
        }
        EntityType::Clan => {
            db.get_clan(&entity_id)?;
        }
    }
    let rel_path = format!("images/{}.{}", new_uuid(), ext);
    let full = s.upload_dir.join(&rel_path);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ApiError(AppError::General(e.to_string())))?;
    }
    std::fs::write(&full, &file_bytes).map_err(|e| ApiError(AppError::General(e.to_string())))?;

    let image = db.add_image(NewImage {
        entity_type: et,
        entity_id,
        file_path: rel_path,
        caption,
        is_cover: false,
    })?;
    Ok(Json(image))
}

// ---------------------------------------------------------------------------
// base64 照片上传（兼容 Android WebView fetch 不支持 multipart 的问题）
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
struct UploadBase64In {
    entity_type: String,
    entity_id: String,
    caption: Option<String>,
    file_name: String,
    file_data: String,
}

async fn upload_image_base64(
    State(s): State<AppState>,
    Json(input): Json<UploadBase64In>,
) -> Result<Json<clantrail_core::Image>> {
    // 解码 base64
    let file_bytes = base64::engine::general_purpose::STANDARD
        .decode(&input.file_data)
        .map_err(|e| ApiError(AppError::InvalidInput(format!("base64 解码失败: {e}"))))?;

    // 魔数校验
    let kind = detect_image_kind(&file_bytes);
    if kind.is_none() {
        return Err(ApiError(AppError::InvalidInput(
            "文件内容不是受支持的图片格式".into(),
        )));
    }

    // 从原始文件名取扩展名
    let ext = std::path::Path::new(&input.file_name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_else(|| "bin".into());
    if !matches!(
        ext.as_str(),
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "heif"
    ) {
        return Err(ApiError(AppError::InvalidInput(format!(
            "不支持的图片类型: .{ext}"
        ))));
    }

    // 文本字段长度限制
    const MAX_ENTITY_ID: usize = 128;
    const MAX_CAPTION: usize = 1024;
    if input.entity_id.len() > MAX_ENTITY_ID {
        return Err(ApiError(AppError::InvalidInput(format!(
            "entity_id 过长（上限 {MAX_ENTITY_ID}）"
        ))));
    }
    if let Some(cap) = &input.caption {
        if cap.len() > MAX_CAPTION {
            return Err(ApiError(AppError::InvalidInput(format!(
                "caption 过长（上限 {MAX_CAPTION}）"
            ))));
        }
    }

    let et = EntityType::from_str(&input.entity_type)
        .ok_or_else(|| ApiError(AppError::InvalidInput(format!("未知 entity_type: {}", input.entity_type))))?;

    // 校验关联实体存在
    let db = s.db()?;
    match et {
        EntityType::Grave => { db.get_grave(&input.entity_id)?; }
        EntityType::Member => { db.get_member(&input.entity_id)?; }
        EntityType::Clan => { db.get_clan(&input.entity_id)?; }
    }

    let rel_path = format!("images/{}.{}", new_uuid(), ext);
    let full = s.upload_dir.join(&rel_path);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ApiError(AppError::General(e.to_string())))?;
    }
    std::fs::write(&full, &file_bytes).map_err(|e| ApiError(AppError::General(e.to_string())))?;

    let image = db.add_image(NewImage {
        entity_type: et,
        entity_id: input.entity_id,
        file_path: rel_path,
        caption: input.caption,
        is_cover: false,
    })?;
    Ok(Json(image))
}

/// 根据文件魔数判断是否为受支持的图片格式。
/// 仅用于上传校验，避免伪造扩展名上传非图片内容。
fn detect_image_kind(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() < 12 {
        return None;
    }
    // JPEG: FF D8 FF
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpg");
    }
    // PNG: 89 50 4E 47 0D 0A 1A 0A
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("png");
    }
    // GIF: 47 49 46 38 (GIF8)
    if bytes.starts_with(b"GIF8") {
        return Some("gif");
    }
    // WEBP: RIFF....WEBP
    if bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }
    // HEIC/HEIF: ....ftyp<brand>...（ISO BMFF）
    if &bytes[4..8] == b"ftyp" {
        let brand = &bytes[8..12];
        if matches!(brand, b"heic" | b"mif1" | b"hevc" | b"avci" | b"hevx") {
            return Some("heic");
        }
    }
    None
}

// 提供已上传图片的静态访问，防目录穿越
async fn serve_image_file(
    State(s): State<AppState>,
    Path(rel): Path<String>,
) -> Result<Response> {
    let base = s
        .upload_dir
        .canonicalize()
        .unwrap_or_else(|_| s.upload_dir.clone());
    let full = s
        .upload_dir
        .join(&rel)
        .canonicalize()
        .map_err(|_| ApiError(AppError::NotFound("image".into())))?;
    if !full.starts_with(&base) {
        return Err(ApiError(AppError::NotFound("image".into())));
    }
    let data = tokio::fs::read(&full)
        .await
        .map_err(|_| ApiError(AppError::NotFound("image".into())))?;
    let mime = mime_type(&full);
    let resp = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .body(Body::from(data))
        .map_err(|e| ApiError(AppError::General(e.to_string())))?;
    Ok(resp)
}

/// 根据扩展名推断 content-type
fn mime_type(p: &std::path::Path) -> &'static str {
    match p
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("heic") => "image/heic",
        Some("heif") => "image/heif",
        _ => "application/octet-stream",
    }
}

// ---------------------------------------------------------------------------
// 搜索
// ---------------------------------------------------------------------------
async fn list_reminders(
    State(s): State<AppState>,
    Query(q): Query<ReminderQuery>,
) -> Result<Json<Vec<clantrail_core::Reminder>>> {
    let days = q.days.unwrap_or(90);
    Ok(Json(paginate(s.db()?.list_reminders(days)?, q.limit, q.offset)))
}

async fn search(
    State(s): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<SearchResults>> {
    // 注意：必须只锁一次。若每个字段各调一次 s.db()?，
    // 第一个锁守卫存活到语句结束，再锁同一 Mutex 会死锁。
    let db = s.db()?;
    let limit = q.limit;
    let offset = q.offset;
    Ok(Json(SearchResults {
        graves: paginate(db.search_graves(&q.keyword)?, limit, offset),
        members: paginate(db.search_members(&q.keyword)?, limit, offset),
        clans: paginate(db.search_clans(&q.keyword)?, limit, offset),
    }))
}

// ---------------------------------------------------------------------------
// 开发种子数据（仅空库时生效，方便联调）
// ---------------------------------------------------------------------------
/// release 构建下的占位 handler：种子端点仅 debug 可用
async fn seed_disabled(State(_s): State<AppState>) -> Result<Json<serde_json::Value>> {
    Err(ApiError(AppError::InvalidInput(
        "seed endpoint is only available in debug builds".into(),
    )))
}

async fn seed_dev_data(State(s): State<AppState>) -> Result<Json<serde_json::Value>> {
    let db = s.db()?;
    if !db.list_clans()?.is_empty() {
        return Ok(Json(serde_json::json!({ "seeded": false, "reason": "db not empty" })));
    }
    let f = db.create_clan(NewClan {
        name: "张氏家族".into(),
        description: Some("明代迁入西安，开枝散叶六代。".into()),
        origin: Some("山西洪洞".into()),
    })?;
    let g1 = db.create_burial_group(NewBurialGroup {
        clan_id: f.id.clone(),
        name: "老坟区".into(),
        description: Some("明代老坟".into()),
    })?;
    let g2 = db.create_burial_group(NewBurialGroup {
        clan_id: f.id.clone(),
        name: "新区".into(),
        description: Some("近现代新立".into()),
    })?;
    let t1 = db.create_grave(NewGrave {
        name: "张氏祖坟".into(),
        latitude: 34.3416,
        longitude: 108.9398,
        address: Some("西安东郊".into()),
        description: Some("明代祖坟，历经六代修缮。".into()),
        burial_group_id: Some(g1.id.clone()),
        clan_id: Some(f.id.clone()),
    })?;
    let t2 = db.create_grave(NewGrave {
        name: "张公新墓".into(),
        latitude: 34.3516,
        longitude: 108.9498,
        address: Some("西安南郊".into()),
        description: Some("1980 年立".into()),
        burial_group_id: Some(g2.id.clone()),
        clan_id: Some(f.id.clone()),
    })?;

    db.create_member(NewMember {
        grave_id: Some(t1.id.clone()),
        clan_id: Some(f.id.clone()),
        name: "张远山".into(),
        title: Some("一世祖".into()),
        birth_date: Some("1420-03-12".into()),
        death_date: Some("1498-11-02".into()),
        biography: Some("明初随军入陕，定居西安东郊，开枝散叶。".into()),
        epitaph: Some("德泽绵长，后世永铭。".into()),
        spouse: Some("李氏".into()),
        is_joint_burial: true,
        children: Some("张大、张二、张三".into()),
        is_alive: false,
        order_index: 1,
    })?;
    db.create_member(NewMember {
        grave_id: Some(t1.id.clone()),
        clan_id: Some(f.id.clone()),
        name: "李氏".into(),
        title: Some("一世祖母".into()),
        birth_date: Some("1425-08-15".into()),
        death_date: Some("1502-12-01".into()),
        biography: Some("随夫定居西安，相夫教子。".into()),
        epitaph: None,
        spouse: Some("张远山".into()),
        is_joint_burial: true,
        children: Some("张大、张二、张三".into()),
        is_alive: false,
        order_index: 2,
    })?;
    db.create_member(NewMember {
        grave_id: Some(t2.id.clone()),
        clan_id: Some(f.id.clone()),
        name: "张守业".into(),
        title: Some("二世祖".into()),
        birth_date: Some("1445-06-20".into()),
        death_date: Some("1512-09-15".into()),
        biography: Some("承父业，置田百亩，兴修家塾。".into()),
        epitaph: None,
        spouse: Some("王氏".into()),
        is_joint_burial: false,
        children: Some("张文、张武".into()),
        is_alive: false,
        order_index: 1,
    })?;

    // 近现代人物：农历换算需在 lunar-lite 支持范围（1900-2100），用于演示忌日提醒。
    db.create_member(NewMember {
        grave_id: Some(t2.id.clone()),
        clan_id: Some(f.id.clone()),
        name: "张建国".into(),
        title: Some("曾祖".into()),
        birth_date: Some("1921-05-10".into()),
        death_date: Some("1998-07-03".into()),
        biography: Some("务农为生，勤俭持家。".into()),
        epitaph: None,
        spouse: Some("刘秀兰".into()),
        is_joint_burial: false,
        children: Some("张志强".into()),
        is_alive: false,
        order_index: 2,
    })?;
    db.create_member(NewMember {
        grave_id: Some(t2.id.clone()),
        clan_id: Some(f.id.clone()),
        name: "张志强".into(),
        title: Some("祖父".into()),
        birth_date: Some("1952-11-22".into()),
        death_date: Some("2021-04-18".into()),
        biography: Some("工厂技师，晚年随子居西安南郊。".into()),
        epitaph: None,
        spouse: None,
        is_joint_burial: false,
        children: Some("张磊".into()),
        is_alive: false,
        order_index: 3,
    })?;

    Ok(Json(serde_json::json!({
        "seeded": true,
        "clan_id": f.id,
        "graves": 2,
        "members": 5,
    })))
}

// ---------------------------------------------------------------------------
// 数据导出/导入（本地备份与恢复）
// ---------------------------------------------------------------------------
const CURRENT_SCHEMA_VERSION: u32 = 4;

#[derive(Serialize, Deserialize)]
struct BackupManifest {
    schema_version: u32,
    exported_at: String,
    app_version: String,
}

/// 获取备份存储目录（{db_dir}/backups），创建目录不存在则创建。
fn backups_dir(db_path: &std::path::Path) -> std::result::Result<std::path::PathBuf, ApiError> {
    let dir = db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("backups");
    std::fs::create_dir_all(&dir)
        .map_err(|e| ApiError(AppError::General(format!("创建备份目录失败: {e}"))))?;
    Ok(dir)
}

/// 导出备份：创建 zip 并落盘到 backups/ 目录，返回文件信息。
async fn create_export(State(s): State<AppState>) -> Result<Json<serde_json::Value>> {
    let upload_dir = s.upload_dir.clone();
    let db_for_snapshot = s.db.clone();
    let db_path = s.db_path.clone();

    let result = tokio::task::spawn_blocking(move || -> std::result::Result<serde_json::Value, ApiError> {
        let mut buf = Vec::new();
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        // manifest.json
        let manifest = BackupManifest {
            schema_version: CURRENT_SCHEMA_VERSION,
            exported_at: chrono::Utc::now().to_rfc3339(),
            app_version: env!("CARGO_PKG_VERSION").into(),
        };
        zip.start_file("manifest.json", options)?;
        zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;

        // 数据库文件：用 VACUUM INTO 生成一致快照
        let temp_dir = tempfile::tempdir()
            .map_err(|e| ApiError(AppError::General(format!("创建临时目录失败: {e}"))))?;
        let snapshot = temp_dir.path().join("clantrail-snapshot.db");
        {
            let db = db_for_snapshot
                .lock()
                .map_err(|_| ApiError(AppError::General("db lock poisoned".into())))?;
            db.create_backup_snapshot(snapshot.to_str().unwrap_or(""))?;
        }
        zip.start_file("clantrail.db", options)?;
        let db_data = std::fs::read(&snapshot)
            .map_err(|e| ApiError(AppError::General(format!("读取数据库快照失败: {e}"))))?;
        zip.write_all(&db_data)?;

        // uploads 目录
        add_dir_to_zip(&mut zip, &upload_dir, "uploads", options)?;

        zip.finish().map_err(|e| ApiError(AppError::General(e.to_string())))?;

        // 落盘到 backups/ 目录
        let filename = format!(
            "clantrail-backup-{}.zip",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        );
        let out_dir = backups_dir(&db_path)?;
        let out_path = out_dir.join(&filename);
        std::fs::write(&out_path, &buf)
            .map_err(|e| ApiError(AppError::General(format!("写入备份文件失败: {e}"))))?;
        let size_bytes = buf.len();

        Ok(serde_json::json!({
            "path": out_path.to_string_lossy(),
            "filename": filename,
            "size_bytes": size_bytes,
        }))
    })
    .await
    .map_err(|e| ApiError(AppError::General(format!("导出任务 panic: {e}"))))?;

    Ok(Json(result?))
}

/// 列出所有已保存的备份文件。
async fn list_backups(State(s): State<AppState>) -> Result<Json<Vec<serde_json::Value>>> {
    let db_path = s.db_path.clone();
    let dir = backups_dir(&db_path)?;
    let mut entries: Vec<serde_json::Value> = Vec::new();
    for entry in std::fs::read_dir(&dir)
        .map_err(|e| ApiError(AppError::General(format!("读取备份目录失败: {e}"))))?
    {
        let entry = entry.map_err(|e| ApiError(AppError::General(e.to_string())))?;
        let meta = entry.metadata().map_err(|e| ApiError(AppError::General(e.to_string())))?;
        if meta.is_file() {
            if let Ok(modified) = meta.modified() {
                let datetime: chrono::DateTime<chrono::Local> = modified.into();
                entries.push(serde_json::json!({
                    "filename": entry.file_name().to_string_lossy(),
                    "size_bytes": meta.len(),
                    "modified_at": datetime.to_rfc3339(),
                }));
            }
        }
    }
    // 按修改时间降序（最新在前）
    entries.sort_by(|a, b| {
        b.get("modified_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .cmp(a.get("modified_at").and_then(|v| v.as_str()).unwrap_or(""))
    });
    Ok(Json(entries))
}

/// 下载/查看备份文件。含路径穿越防护。
async fn download_backup(
    State(s): State<AppState>,
    Path(filename): Path<String>,
) -> Result<Response> {
    let db_path = s.db_path.clone();
    let dir = backups_dir(&db_path)?;
    let base = dir.canonicalize().unwrap_or(dir);
    let full = base.join(&filename);
    let canonical = full.canonicalize().map_err(|_| {
        ApiError(AppError::NotFound(format!("backup {filename}")))
    })?;
    if !canonical.starts_with(&base) {
        return Err(ApiError(AppError::NotFound(format!("backup {filename}"))));
    }
    let data = std::fs::read(&canonical)
        .map_err(|_| ApiError(AppError::NotFound(format!("backup {filename}"))))?;
    let resp = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(Body::from(data))
        .map_err(|e| ApiError(AppError::General(e.to_string())))?;
    Ok(resp)
}

/// 删除已保存的备份文件。
async fn delete_backup(
    State(s): State<AppState>,
    Path(filename): Path<String>,
) -> Result<StatusCode> {
    let db_path = s.db_path.clone();
    let dir = backups_dir(&db_path)?;
    let base = dir.canonicalize().unwrap_or(dir);
    let full = base.join(&filename);
    let canonical = full.canonicalize().map_err(|_| {
        ApiError(AppError::NotFound(format!("backup {filename}")))
    })?;
    if !canonical.starts_with(&base) {
        return Err(ApiError(AppError::NotFound(format!("backup {filename}"))));
    }
    std::fs::remove_file(&canonical)
        .map_err(|_| ApiError(AppError::NotFound(format!("backup {filename}"))))?;
    Ok(StatusCode::NO_CONTENT)
}

fn add_dir_to_zip<W: std::io::Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    dir: &std::path::Path,
    prefix: &str,
    options: zip::write::SimpleFileOptions,
) -> std::result::Result<(), ApiError> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let rel = entry.path().strip_prefix(dir).map_err(|e| ApiError(AppError::General(e.to_string())))?;
        let name = format!("{}/{}", prefix.trim_end_matches('/'), rel.to_string_lossy().replace('\\', "/"));
        zip.start_file(name, options)?;
        let data = std::fs::read(entry.path())
            .map_err(|e| ApiError(AppError::General(format!("读取文件失败: {e}"))))?;
        zip.write_all(&data)?;
    }
    Ok(())
}

/// 导入备份 zip：校验、备份当前数据、替换数据库与 uploads、重新打开数据库。
async fn import_backup(State(s): State<AppState>, mut multipart: Multipart) -> Result<Json<serde_json::Value>> {
    let mut file_bytes: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError(AppError::InvalidInput(e.to_string())))?
    {
        if field.name().map(|n| n == "file").unwrap_or(false) {
            file_bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| ApiError(AppError::InvalidInput(e.to_string())))?
                    .to_vec(),
            );
        }
    }
    let file_bytes = file_bytes.ok_or_else(|| ApiError(AppError::InvalidInput("缺少备份文件".into())))?;

    let db_path = s.db_path.clone();
    let upload_dir = s.upload_dir.clone();

    tokio::task::spawn_blocking(move || -> std::result::Result<serde_json::Value, ApiError> {
        let temp_dir = tempfile::tempdir()
            .map_err(|e| ApiError(AppError::General(format!("创建临时目录失败: {e}"))))?;
        let temp_path = temp_dir.path();

        // 解压
        let reader = std::io::Cursor::new(file_bytes);
        let mut zip = zip::ZipArchive::new(reader)
            .map_err(|e| ApiError(AppError::InvalidInput(format!("无法解析 zip: {e}"))))?;
        zip.extract(temp_path)
            .map_err(|e| ApiError(AppError::InvalidInput(format!("解压失败: {e}"))))?;

        // 校验 manifest
        let manifest: BackupManifest = {
            let p = temp_path.join("manifest.json");
            let text = std::fs::read_to_string(&p)
                .map_err(|e| ApiError(AppError::InvalidInput(format!("缺少 manifest.json: {e}"))))?;
            serde_json::from_str(&text)
                .map_err(|e| ApiError(AppError::InvalidInput(format!("manifest 格式错误: {e}"))))?
        };
        if manifest.schema_version > CURRENT_SCHEMA_VERSION {
            return Err(ApiError(AppError::InvalidInput(format!(
                "备份 schema 版本 {} 高于当前支持 {}",
                manifest.schema_version, CURRENT_SCHEMA_VERSION
            ))));
        }

        let backup_db = temp_path.join("clantrail.db");
        if !backup_db.exists() {
            return Err(ApiError(AppError::InvalidInput("备份中缺少 clantrail.db".into())));
        }

        // 先验证备份数据库能打开
        {
            let _ = ClanTrailDb::open(backup_db.to_str().unwrap_or(""))
                .map_err(|e| ApiError(AppError::InvalidInput(format!("备份数据库损坏: {e}"))))?;
        }

        // 关闭当前连接、备份并替换；任一步失败则回滚到原库。
        let backup_dir;
        {
            let mut db = s.db()?;
            db.close_to_memory()
                .map_err(|e| ApiError(AppError::General(format!("关闭数据库失败: {e}"))))?;

            let now = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
            backup_dir = db_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join(format!(".clantrail-backup-{}", now));
            std::fs::create_dir_all(&backup_dir)
                .map_err(|e| ApiError(AppError::General(format!("创建备份目录失败: {e}"))))?;

            // 备份当前 db
            if db_path.exists() {
                std::fs::copy(&db_path, backup_dir.join("clantrail.db"))
                    .map_err(|e| ApiError(AppError::General(format!("备份数据库失败: {e}"))))?;
            }
            // 备份当前 uploads
            if upload_dir.exists() {
                let backup_uploads = backup_dir.join("uploads");
                copy_dir_all(&upload_dir, &backup_uploads)
                    .map_err(|e| ApiError(AppError::General(format!("备份 uploads 失败: {e}"))))?;
            }

            // 尝试替换并重开；失败则回滚
            let replace_result: std::result::Result<(), ApiError> = (|| {
                // 替换 db 文件
                std::fs::copy(&backup_db, &db_path)
                    .map_err(|e| ApiError(AppError::General(format!("替换数据库失败: {e}"))))?;
                // 临时 rename 旧 uploads，避免中途失败破坏原目录
                let old_uploads_tmp = upload_dir.with_extension("old-uploads-tmp");
                if upload_dir.exists() {
                    std::fs::rename(&upload_dir, &old_uploads_tmp)
                        .map_err(|e| ApiError(AppError::General(format!("移动旧 uploads 失败: {e}"))))?;
                }
                let temp_uploads = temp_path.join("uploads");
                if temp_uploads.exists() {
                    copy_dir_all(&temp_uploads, &upload_dir)
                        .map_err(|e| ApiError(AppError::General(format!("恢复 uploads 失败: {e}"))))?;
                } else {
                    std::fs::create_dir_all(&upload_dir)
                        .map_err(|e| ApiError(AppError::General(format!("创建 uploads 失败: {e}"))))?;
                }
                // 清理临时旧 uploads
                if old_uploads_tmp.exists() {
                    std::fs::remove_dir_all(&old_uploads_tmp).ok();
                }
                Ok(())
            })();

            if let Err(e) = replace_result {
                // 回滚：把备份的 db 与 uploads 恢复回去，并重开原库
                let _ = std::fs::copy(backup_dir.join("clantrail.db"), &db_path);
                if backup_dir.join("uploads").exists() {
                    let _ = std::fs::remove_dir_all(&upload_dir);
                    let _ = copy_dir_all(&backup_dir.join("uploads"), &upload_dir);
                }
                let _ = db.reopen(db_path.to_str().unwrap_or(""));
                return Err(e);
            }

            // 重新打开新数据库
            db.reopen(db_path.to_str().unwrap_or(""))
                .map_err(|e| ApiError(AppError::General(format!("重新打开数据库失败: {e}"))))?;
            // 清理历史备份，仅保留最近 5 份，避免磁盘堆积
            if let Some(parent) = db_path.parent() {
                cleanup_old_backups(parent, 5);
            }
        }

        Ok(serde_json::json!({
            "restored": true,
            "schema_version": manifest.schema_version,
            "exported_at": manifest.exported_at,
            "backup_dir": backup_dir.to_string_lossy(),
        }))
    })
    .await
    .map_err(|e| ApiError(AppError::General(format!("导入任务 panic: {e}"))))?
    .map(Json)
}

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// 清理形如 `.clantrail-backup-{ts}` 的历史备份目录，仅保留最近 keep 份，避免磁盘堆积。
fn cleanup_old_backups(parent: &std::path::Path, keep: usize) {
    let prefix = ".clantrail-backup-";
    let mut dirs: Vec<std::path::PathBuf> = std::fs::read_dir(parent)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type()
                .map(|t| t.is_dir())
                .unwrap_or(false)
                && e.file_name().to_string_lossy().starts_with(prefix)
        })
        .map(|e| e.path())
        .collect();
    // 目录名含时间戳，字典序即时间序，按降序后跳过最近的 keep 份
    dirs.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    for old in dirs.into_iter().skip(keep) {
        let _ = std::fs::remove_dir_all(&old);
    }
}

// ---------------------------------------------------------------------------
// 更新请求体
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
struct UpdateClanBody {
    name: Option<String>,
    description: Option<String>,
    origin: Option<String>,
}

#[derive(Deserialize)]
struct UpdateGraveBody {
    name: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    address: Option<String>,
    description: Option<String>,
    burial_group_id: Option<String>,
    clan_id: Option<String>,
}

#[derive(Deserialize)]
struct UpdateMemberBody {
    name: Option<String>,
    title: Option<String>,
    birth_date: Option<String>,
    death_date: Option<String>,
    biography: Option<String>,
    epitaph: Option<String>,
    spouse: Option<String>,
    is_joint_burial: Option<bool>,
    children: Option<String>,
    order_index: Option<i32>,
    clan_id: Option<String>,
    is_alive: Option<bool>,
}

#[derive(Deserialize)]
struct UpdateImageBody {
    is_cover: bool,
}

#[derive(Serialize)]
struct SearchResults {
    graves: Vec<clantrail_core::Grave>,
    members: Vec<clantrail_core::Grave>,
    clans: Vec<clantrail_core::Clan>,
}

#[derive(Serialize)]
struct GraphResponse {
    members: Vec<clantrail_core::Member>,
    edges: Vec<clantrail_core::Edge>,
}
