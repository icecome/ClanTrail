use std::path::PathBuf;

use clantrail_server::{build_router, AppState};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "clantrail_server=info,tower_http=info".into()),
        )
        .init();

    let db_path: PathBuf = std::env::var("CLANTRAIL_DB")
        .unwrap_or_else(|_| "clantrail.db".into())
        .into();
    let db = clantrail_core::ClanTrailDb::open(db_path.to_str().unwrap_or("clantrail.db"))
        .expect("failed to open database");

    // 照片本地存储目录：CLANTRAIL_UPLOAD_DIR 可指定，默认 ./uploads。
    // DB 仅存相对路径（如 images/<uuid>.<ext>），备份时随 uploads/ 整体迁移。
    let upload_dir = std::env::var("CLANTRAIL_UPLOAD_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("uploads"));
    std::fs::create_dir_all(&upload_dir).expect("failed to create upload dir");

    let state = AppState::new(
        db,
        db_path.canonicalize().unwrap_or_else(|_| db_path.clone()),
        upload_dir.canonicalize().unwrap_or_else(|_| upload_dir.clone()),
    );

    let app = build_router(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".into());
    // 默认绑定回环地址（127.0.0.1），避免暴露到局域网/公网。
    // 可通过环境变量 CLANTRAIL_HOST 覆盖（如 "0.0.0.0" 允许局域网访问）。
    let host = std::env::var("CLANTRAIL_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");
    tracing::info!("ClanTrail server listening on http://{addr}");
    axum::serve(listener, app).await.expect("server error");
}
