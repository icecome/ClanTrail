use std::io::Write as _;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use axum::{
    body::Body,
    extract::{multipart::Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tomb_keeper_core::{
    new_uuid, AppError, EntityType, NewFamily, NewPerson, NewPhoto, NewRelation, NewTomb,
    NewTombGroup, TombKeeperDb,
};

pub type Result<T> = std::result::Result<T, ApiError>;

/// rusqlite::Connection 不是 Sync，用 Mutex 包装以通过 axum 的 Send+Sync 约束。
/// 所有数据库操作都是同步短调用，不跨 await 持锁，std::sync::Mutex 足够。
type SharedDb = Arc<Mutex<TombKeeperDb>>;

#[derive(Clone)]
pub struct AppState {
    pub db: SharedDb,
    /// SQLite 数据库文件路径（绝对路径）。导出/导入时需要。
    pub db_path: PathBuf,
    /// 照片本地存储目录（绝对路径）。DB 只存相对路径，便于备份迁移。
    pub upload_dir: PathBuf,
}

impl AppState {
    pub fn new(db: TombKeeperDb, db_path: PathBuf, upload_dir: PathBuf) -> Self {
        AppState {
            db: Arc::new(Mutex::new(db)),
            db_path,
            upload_dir,
        }
    }

    pub fn db(&self) -> Result<MutexGuard<'_, TombKeeperDb>> {
        self.db
            .lock()
            .map_err(|_| ApiError(AppError::General("db lock poisoned".into())))
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SearchQuery {
    keyword: String,
}

#[derive(Debug, Deserialize)]
struct ReminderQuery {
    days: Option<u32>,
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
            AppError::Database(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::Serde(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::General(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
        };
        (status, Json(serde_json::json!({ "error": msg }))).into_response()
    }
}

/// 构建路由（不绑定端口）。供独立二进制与 Tauri 内嵌后端共用。
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        // Family
        .route("/api/families", get(list_families).post(create_family))
        .route(
            "/api/families/:id",
            get(get_family).put(update_family).delete(delete_family),
        )
        .route("/api/families/:id/groups", get(list_groups))
        .route("/api/families/:id/tombs", get(list_family_tombs))
        // TombGroup
        .route("/api/tomb-groups", post(create_group))
        .route("/api/tomb-groups/:id", delete(delete_group))
        // Tomb
        .route("/api/tombs", get(list_tombs).post(create_tomb))
        .route(
            "/api/tombs/:id",
            get(get_tomb).put(update_tomb).delete(delete_tomb),
        )
        .route("/api/tombs/:id/persons", get(list_persons))
        .route("/api/tombs/:id/photos", get(list_photos))
        // Person
        .route("/api/persons", post(create_person))
        .route("/api/persons/:id", put(update_person).delete(delete_person))
        // Relation
        .route("/api/persons/:id/relations", get(list_person_relations).post(create_person_relation))
        .route("/api/relations/:id", delete(delete_relation))
        // Photo
        .route("/api/photos", post(create_photo))
        .route("/api/photos/upload", post(upload_photo))
        .route("/api/photos/files/*path", get(serve_photo_file))
        .route("/api/photos/:id", put(update_photo).delete(delete_photo))
        // 数据导出/导入（本地备份与恢复）
        .route("/api/export", get(export_backup))
        .route("/api/import", post(import_backup))
        // 祭祀/忌日提醒
        .route("/api/reminders", get(list_reminders))
        // 搜索
        .route("/api/search", get(search))
        // 开发种子数据
        .route("/api/dev/seed", post(seed_dev_data))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

// ---------------------------------------------------------------------------
// Family
// ---------------------------------------------------------------------------
async fn list_families(State(s): State<AppState>) -> Result<Json<Vec<tomb_keeper_core::Family>>> {
    Ok(Json(s.db()?.list_families()?))
}

async fn create_family(
    State(s): State<AppState>,
    Json(input): Json<NewFamily>,
) -> Result<Json<tomb_keeper_core::Family>> {
    Ok(Json(s.db()?.create_family(input)?))
}

async fn get_family(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<tomb_keeper_core::Family>> {
    Ok(Json(s.db()?.get_family(&id)?))
}

async fn update_family(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateFamilyBody>,
) -> Result<Json<tomb_keeper_core::Family>> {
    Ok(Json(s.db()?.update_family(&id, body.name, body.description, body.origin)?))
}

async fn delete_family(State(s): State<AppState>, Path(id): Path<String>) -> Result<StatusCode> {
    s.db()?.delete_family(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_groups(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<tomb_keeper_core::TombGroup>>> {
    Ok(Json(s.db()?.list_groups_by_family(&id)?))
}

async fn list_family_tombs(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<tomb_keeper_core::Tomb>>> {
    Ok(Json(s.db()?.list_tombs_by_family(&id)?))
}

// ---------------------------------------------------------------------------
// TombGroup
// ---------------------------------------------------------------------------
async fn create_group(
    State(s): State<AppState>,
    Json(input): Json<NewTombGroup>,
) -> Result<Json<tomb_keeper_core::TombGroup>> {
    Ok(Json(s.db()?.create_tomb_group(input)?))
}

async fn delete_group(State(s): State<AppState>, Path(id): Path<String>) -> Result<StatusCode> {
    s.db()?.delete_tomb_group(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Tomb
// ---------------------------------------------------------------------------
async fn list_tombs(State(s): State<AppState>) -> Result<Json<Vec<tomb_keeper_core::Tomb>>> {
    Ok(Json(s.db()?.list_tombs()?))
}

async fn create_tomb(
    State(s): State<AppState>,
    Json(input): Json<NewTomb>,
) -> Result<Json<tomb_keeper_core::Tomb>> {
    Ok(Json(s.db()?.create_tomb(input)?))
}

async fn get_tomb(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<tomb_keeper_core::Tomb>> {
    Ok(Json(s.db()?.get_tomb(&id)?))
}

async fn update_tomb(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateTombBody>,
) -> Result<Json<tomb_keeper_core::Tomb>> {
    Ok(Json(s.db()?.update_tomb(
        &id,
        body.name,
        body.latitude,
        body.longitude,
        body.address,
        body.description,
        body.group_id,
        body.family_id,
    )?))
}

async fn delete_tomb(State(s): State<AppState>, Path(id): Path<String>) -> Result<StatusCode> {
    s.db()?.delete_tomb(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_persons(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<tomb_keeper_core::Person>>> {
    Ok(Json(s.db()?.list_persons_by_tomb(&id)?))
}

async fn list_photos(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<tomb_keeper_core::Photo>>> {
    Ok(Json(s.db()?.list_photos_by_entity(
        tomb_keeper_core::EntityType::Tomb,
        &id,
    )?))
}

// ---------------------------------------------------------------------------
// Person
// ---------------------------------------------------------------------------
async fn create_person(
    State(s): State<AppState>,
    Json(input): Json<NewPerson>,
) -> Result<Json<tomb_keeper_core::Person>> {
    Ok(Json(s.db()?.create_person(input)?))
}

async fn update_person(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdatePersonBody>,
) -> Result<Json<tomb_keeper_core::Person>> {
    Ok(Json(s.db()?.update_person(
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
    )?))
}

async fn delete_person(State(s): State<AppState>, Path(id): Path<String>) -> Result<StatusCode> {
    s.db()?.delete_person(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Relation
// ---------------------------------------------------------------------------
async fn list_person_relations(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<tomb_keeper_core::Relation>>> {
    Ok(Json(s.db()?.list_relations_by_person(&id)?))
}

async fn create_person_relation(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<NewRelation>,
) -> Result<Json<Vec<tomb_keeper_core::Relation>>> {
    if input.person_id != id {
        return Err(ApiError(AppError::InvalidInput(
            "person_id in path does not match body".into(),
        )));
    }
    Ok(Json(s.db()?.create_relation(input)?))
}

async fn delete_relation(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    s.db()?.delete_relation(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Photo
// ---------------------------------------------------------------------------
async fn create_photo(
    State(s): State<AppState>,
    Json(input): Json<NewPhoto>,
) -> Result<Json<tomb_keeper_core::Photo>> {
    Ok(Json(s.db()?.add_photo(input)?))
}

async fn delete_photo(State(s): State<AppState>, Path(id): Path<String>) -> Result<StatusCode> {
    let rel_path = {
        let db = s.db()?;
        let photo = db.get_photo(&id)?;
        db.delete_photo(&id)?;
        photo.file_path
    };
    // best-effort 删除物理文件，失败不影响接口返回
    let full = s.upload_dir.join(&rel_path);
    let _ = tokio::fs::remove_file(&full).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn update_photo(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdatePhotoBody>,
) -> Result<Json<tomb_keeper_core::Photo>> {
    Ok(Json(s.db()?.set_photo_cover(&id, body.is_cover)?))
}

// ---------------------------------------------------------------------------
// 真实照片上传：multipart/form-data（file, entity_type, entity_id, caption?）
// ---------------------------------------------------------------------------
async fn upload_photo(
    State(s): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<tomb_keeper_core::Photo>> {
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
            Some("entity_type") => entity_type = field.text().await.ok(),
            Some("entity_id") => entity_id = field.text().await.ok(),
            Some("caption") => caption = field.text().await.ok(),
            _ => {}
        }
    }

    let file_bytes =
        file_bytes.ok_or_else(|| ApiError(AppError::InvalidInput("缺少上传文件".into())))?;
    let entity_type =
        entity_type.ok_or_else(|| ApiError(AppError::InvalidInput("缺少 entity_type".into())))?;
    let entity_id =
        entity_id.ok_or_else(|| ApiError(AppError::InvalidInput("缺少 entity_id".into())))?;
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

    // 校验关联实体存在（持同一把锁，不跨 await）
    let db = s.db()?;
    match et {
        EntityType::Tomb => {
            db.get_tomb(&entity_id)?;
        }
        EntityType::Person => {
            db.get_person(&entity_id)?;
        }
        EntityType::Family => {
            db.get_family(&entity_id)?;
        }
    }
    let rel_path = format!("photos/{}.{}", new_uuid(), ext);
    let full = s.upload_dir.join(&rel_path);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ApiError(AppError::General(e.to_string())))?;
    }
    std::fs::write(&full, &file_bytes).map_err(|e| ApiError(AppError::General(e.to_string())))?;

    let photo = db.add_photo(NewPhoto {
        entity_type: et,
        entity_id,
        file_path: rel_path,
        caption,
        is_cover: false,
    })?;
    Ok(Json(photo))
}

// 提供已上传图片的静态访问，防目录穿越
async fn serve_photo_file(
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
        .map_err(|_| ApiError(AppError::NotFound("photo".into())))?;
    if !full.starts_with(&base) {
        return Err(ApiError(AppError::NotFound("photo".into())));
    }
    let data = tokio::fs::read(&full)
        .await
        .map_err(|_| ApiError(AppError::NotFound("photo".into())))?;
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
) -> Result<Json<Vec<tomb_keeper_core::Reminder>>> {
    let days = q.days.unwrap_or(90);
    Ok(Json(s.db()?.list_reminders(days)?))
}

async fn search(
    State(s): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<SearchResults>> {
    // 注意：必须只锁一次。若每个字段各调一次 s.db()?，
    // 第一个锁守卫存活到语句结束，再锁同一 Mutex 会死锁。
    let db = s.db()?;
    Ok(Json(SearchResults {
        tombs: db.search_tombs(&q.keyword)?,
        persons: db.search_persons(&q.keyword)?,
        families: db.search_families(&q.keyword)?,
    }))
}

// ---------------------------------------------------------------------------
// 开发种子数据（仅空库时生效，方便联调）
// ---------------------------------------------------------------------------
async fn seed_dev_data(State(s): State<AppState>) -> Result<Json<serde_json::Value>> {
    let db = s.db()?;
    if !db.list_families()?.is_empty() {
        return Ok(Json(serde_json::json!({ "seeded": false, "reason": "db not empty" })));
    }
    let f = db.create_family(NewFamily {
        name: "张氏家族".into(),
        description: Some("明代迁入西安，开枝散叶六代。".into()),
        origin: Some("山西洪洞".into()),
    })?;
    let g1 = db.create_tomb_group(NewTombGroup {
        family_id: f.id.clone(),
        name: "老坟区".into(),
        description: Some("明代老坟".into()),
    })?;
    let g2 = db.create_tomb_group(NewTombGroup {
        family_id: f.id.clone(),
        name: "新区".into(),
        description: Some("近现代新立".into()),
    })?;
    let t1 = db.create_tomb(NewTomb {
        name: "张氏祖坟".into(),
        latitude: 34.3416,
        longitude: 108.9398,
        address: Some("西安东郊".into()),
        description: Some("明代祖坟，历经六代修缮。".into()),
        group_id: Some(g1.id.clone()),
        family_id: Some(f.id.clone()),
    })?;
    let t2 = db.create_tomb(NewTomb {
        name: "张公新墓".into(),
        latitude: 34.3516,
        longitude: 108.9498,
        address: Some("西安南郊".into()),
        description: Some("1980 年立".into()),
        group_id: Some(g2.id.clone()),
        family_id: Some(f.id.clone()),
    })?;

    db.create_person(NewPerson {
        tomb_id: t1.id.clone(),
        name: "张远山".into(),
        title: Some("一世祖".into()),
        birth_date: Some("1420-03-12".into()),
        death_date: Some("1498-11-02".into()),
        biography: Some("明初随军入陕，定居西安东郊，开枝散叶。".into()),
        epitaph: Some("德泽绵长，后世永铭。".into()),
        spouse: Some("李氏".into()),
        is_joint_burial: true,
        children: Some("张大、张二、张三".into()),
        order_index: 1,
    })?;
    db.create_person(NewPerson {
        tomb_id: t1.id.clone(),
        name: "李氏".into(),
        title: Some("一世祖母".into()),
        birth_date: Some("1425-08-15".into()),
        death_date: Some("1502-12-01".into()),
        biography: Some("随夫定居西安，相夫教子。".into()),
        epitaph: None,
        spouse: Some("张远山".into()),
        is_joint_burial: true,
        children: Some("张大、张二、张三".into()),
        order_index: 2,
    })?;
    db.create_person(NewPerson {
        tomb_id: t2.id.clone(),
        name: "张守业".into(),
        title: Some("二世祖".into()),
        birth_date: Some("1445-06-20".into()),
        death_date: Some("1512-09-15".into()),
        biography: Some("承父业，置田百亩，兴修家塾。".into()),
        epitaph: None,
        spouse: Some("王氏".into()),
        is_joint_burial: false,
        children: Some("张文、张武".into()),
        order_index: 1,
    })?;

    // 近现代人物：农历换算需在 lunar-lite 支持范围（1900-2100），用于演示忌日提醒。
    db.create_person(NewPerson {
        tomb_id: t2.id.clone(),
        name: "张建国".into(),
        title: Some("曾祖".into()),
        birth_date: Some("1921-05-10".into()),
        death_date: Some("1998-07-03".into()),
        biography: Some("务农为生，勤俭持家。".into()),
        epitaph: None,
        spouse: Some("刘秀兰".into()),
        is_joint_burial: false,
        children: Some("张志强".into()),
        order_index: 2,
    })?;
    db.create_person(NewPerson {
        tomb_id: t2.id.clone(),
        name: "张志强".into(),
        title: Some("祖父".into()),
        birth_date: Some("1952-11-22".into()),
        death_date: Some("2021-04-18".into()),
        biography: Some("工厂技师，晚年随子居西安南郊。".into()),
        epitaph: None,
        spouse: None,
        is_joint_burial: false,
        children: Some("张磊".into()),
        order_index: 3,
    })?;

    Ok(Json(serde_json::json!({
        "seeded": true,
        "family_id": f.id,
        "tombs": 2,
        "persons": 5,
    })))
}

// ---------------------------------------------------------------------------
// 数据导出/导入（本地备份与恢复）
// ---------------------------------------------------------------------------
const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct BackupManifest {
    schema_version: u32,
    exported_at: String,
    app_version: String,
}

/// 导出完整备份：SQLite 数据库 + uploads 目录，打包为 zip 下载。
async fn export_backup(State(s): State<AppState>) -> Result<Response> {
    let db_path = s.db_path.clone();
    let upload_dir = s.upload_dir.clone();

    let zip_bytes = tokio::task::spawn_blocking(move || -> std::result::Result<Vec<u8>, ApiError> {
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

        // 数据库文件
        zip.start_file("tomb-keeper.db", options)?;
        let db_data = std::fs::read(&db_path)
            .map_err(|e| ApiError(AppError::General(format!("读取数据库失败: {e}"))))?;
        zip.write_all(&db_data)?;

        // uploads 目录（保持相对路径 uploads/...）
        add_dir_to_zip(&mut zip, &upload_dir, "uploads", options)?;

        zip.finish().map_err(|e| ApiError(AppError::General(e.to_string())))?;
        Ok(buf)
    })
    .await
    .map_err(|e| ApiError(AppError::General(format!("导出任务 panic: {e}"))))?;

    let zip_bytes = zip_bytes?;
    let filename = format!(
        "tomb-keeper-backup-{}.zip",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );
    let resp = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(Body::from(zip_bytes))
        .map_err(|e| ApiError(AppError::General(e.to_string())))?;
    Ok(resp)
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

        let backup_db = temp_path.join("tomb-keeper.db");
        if !backup_db.exists() {
            return Err(ApiError(AppError::InvalidInput("备份中缺少 tomb-keeper.db".into())));
        }

        // 先验证备份数据库能打开
        {
            let _ = TombKeeperDb::open(backup_db.to_str().unwrap_or(""))
                .map_err(|e| ApiError(AppError::InvalidInput(format!("备份数据库损坏: {e}"))))?;
        }

        // 关闭当前连接、备份并替换
        let backup_dir;
        {
            let mut db = s.db()?;
            db.close_to_memory()
                .map_err(|e| ApiError(AppError::General(format!("关闭数据库失败: {e}"))))?;

            let now = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
            backup_dir = db_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join(format!(".tomb-keeper-backup-{}", now));
            std::fs::create_dir_all(&backup_dir)
                .map_err(|e| ApiError(AppError::General(format!("创建备份目录失败: {e}"))))?;

            // 备份当前 db
            if db_path.exists() {
                std::fs::copy(&db_path, backup_dir.join("tomb-keeper.db"))
                    .map_err(|e| ApiError(AppError::General(format!("备份数据库失败: {e}"))))?;
            }
            // 备份当前 uploads
            if upload_dir.exists() {
                let backup_uploads = backup_dir.join("uploads");
                copy_dir_all(&upload_dir, &backup_uploads)
                    .map_err(|e| ApiError(AppError::General(format!("备份 uploads 失败: {e}"))))?;
            }

            // 替换 db 文件
            std::fs::copy(&backup_db, &db_path)
                .map_err(|e| ApiError(AppError::General(format!("替换数据库失败: {e}"))))?;

            // 替换 uploads 目录
            if upload_dir.exists() {
                std::fs::remove_dir_all(&upload_dir)
                    .map_err(|e| ApiError(AppError::General(format!("删除旧 uploads 失败: {e}"))))?;
            }
            let temp_uploads = temp_path.join("uploads");
            if temp_uploads.exists() {
                copy_dir_all(&temp_uploads, &upload_dir)
                    .map_err(|e| ApiError(AppError::General(format!("恢复 uploads 失败: {e}"))))?;
            } else {
                std::fs::create_dir_all(&upload_dir)
                    .map_err(|e| ApiError(AppError::General(format!("创建 uploads 失败: {e}"))))?;
            }

            // 重新打开新数据库
            db.reopen(db_path.to_str().unwrap_or(""))
                .map_err(|e| ApiError(AppError::General(format!("重新打开数据库失败: {e}"))))?;
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

// ---------------------------------------------------------------------------
// 更新请求体
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
struct UpdateFamilyBody {
    name: Option<String>,
    description: Option<String>,
    origin: Option<String>,
}

#[derive(Deserialize)]
struct UpdateTombBody {
    name: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    address: Option<String>,
    description: Option<String>,
    group_id: Option<String>,
    family_id: Option<String>,
}

#[derive(Deserialize)]
struct UpdatePersonBody {
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
}

#[derive(Deserialize)]
struct UpdatePhotoBody {
    is_cover: bool,
}

#[derive(Serialize)]
struct SearchResults {
    tombs: Vec<tomb_keeper_core::Tomb>,
    persons: Vec<tomb_keeper_core::Tomb>,
    families: Vec<tomb_keeper_core::Family>,
}
