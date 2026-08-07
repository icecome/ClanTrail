use std::path::PathBuf;

use tomb_keeper_server::{build_router, AppState};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tomb_keeper_server=info,tower_http=info".into()),
        )
        .init();

    let db_path: PathBuf = std::env::var("TOMB_KEEPER_DB")
        .unwrap_or_else(|_| "tomb-keeper.db".into())
        .into();
    let db = tomb_keeper_core::TombKeeperDb::open(db_path.to_str().unwrap_or("tomb-keeper.db"))
        .expect("failed to open database");

    // 照片本地存储目录：TOMB_KEEPER_UPLOAD_DIR 可指定，默认 ./uploads。
    // DB 仅存相对路径（如 photos/<uuid>.<ext>），备份时随 uploads/ 整体迁移。
    let upload_dir = std::env::var("TOMB_KEEPER_UPLOAD_DIR")
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
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");
    tracing::info!("Tomb Keeper server listening on http://{addr}");
    axum::serve(listener, app).await.expect("server error");
}
