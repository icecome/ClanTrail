use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context = tauri::generate_context!();
    tauri::Builder::default()
        .setup(|app| {
            // 后端数据放在 App 私有数据目录，避免依赖当前工作目录
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            std::fs::create_dir_all(&data_dir).ok();
            let db_path = data_dir.join("tomb-keeper.db");
            let upload_dir = data_dir.join("uploads");

            let db = tomb_keeper_core::TombKeeperDb::open(db_path.to_str().unwrap_or("tomb-keeper.db"))
                .expect("failed to open database");
            let state = tomb_keeper_server::AppState::new(
                db,
                db_path.clone(),
                upload_dir.clone(),
            );
            let router = tomb_keeper_server::build_router(state);

            // 内嵌 Axum 后端，监听本机回环，供 WebView 直连
            let addr: std::net::SocketAddr = "127.0.0.1:8080".parse().unwrap();
            tauri::async_runtime::spawn(async move {
                match tokio::net::TcpListener::bind(addr).await {
                    Ok(listener) => {
                        let _ = axum::serve(listener, router).await;
                    }
                    Err(e) => {
                        eprintln!("failed to bind embedded server on {addr}: {e}");
                    }
                }
            });

            Ok(())
        })
        .run(context)
        .expect("error while running tauri application");
}
