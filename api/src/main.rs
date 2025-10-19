use axum::{
    extract::Query,
    http::{Method, StatusCode},
    response::{IntoResponse, Redirect},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    net::IpAddr,
    sync::{Arc, Mutex},
};
use tokio::signal;
use tower_http::cors::{Any, CorsLayer};

#[derive(Default, Serialize, Deserialize, Clone)]
struct Stats {
    requests: u64,
    blocks: u64,
    passes: u64,
}

const STATS_FILE: &str = "redblock-stats.json";

fn load_stats() -> Stats {
    if let Ok(raw) = fs::read_to_string(STATS_FILE) {
        if let Ok(parsed) = serde_json::from_str::<Stats>(&raw) {
            return parsed;
        }
    }
    Stats::default()
}

fn save_stats(stats: &Stats) {
    let tmp = format!("{}.tmp", STATS_FILE);
    if let Ok(data) = serde_json::to_string(stats) {
        if let Err(e) = fs::write(&tmp, data) {
            eprintln!("bummer. the stats write failed: {e}");
            return;
        }
        if let Err(e) = fs::rename(&tmp, STATS_FILE) {
            eprintln!("bummer. the stats rename failed: {e}");
        }
    }
}

use std::ffi::CString;
use std::os::raw::c_char;
extern "C" {
    fn ipset_test_member(setname: *const c_char, elem: *const c_char) -> i32;
}

fn test_ip(set_name: &str, ip: &str) -> bool {
    let set_c = CString::new(set_name).unwrap();
    let ip_c = CString::new(ip).unwrap();
    let result = unsafe { ipset_test_member(set_c.as_ptr(), ip_c.as_ptr()) };
    result == 1
}

async fn handle_test(
    Query(params): Query<HashMap<String, String>>,
    stats: Arc<Mutex<Stats>>,
) -> impl IntoResponse {
    let ip = match params.get("ip") {
        Some(i) => i.trim(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "missing ?ip=" })),
            )
        }
    };

    if ip.parse::<IpAddr>().is_err() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "invalid ip" })),
        );
    }

    let set_name = if ip.contains(':') { "blocked6" } else { "blocked4" };

    let set = set_name.to_string();
    let ip_str = ip.to_string();
    let blocked = tokio::task::spawn_blocking(move || test_ip(&set, &ip_str))
        .await
        .unwrap_or(false);

    {
        let mut s = stats.lock().unwrap();
        s.requests += 1;
        if blocked {
            s.blocks += 1;
        } else {
            s.passes += 1;
        }
        save_stats(&s);
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "blocked": blocked })),
    )
}

async fn handle_stats() -> impl IntoResponse {
    let s = load_stats();
    let body = format!(
        "Requests: {}\nBlocks: {}\nPasses: {}\n",
        s.requests, s.blocks, s.passes
    );
    ([("Content-Type", "text/plain; charset=utf-8")], body)
}

async fn handle_root() -> impl IntoResponse {
    Redirect::temporary("https://dispherical.com/tools/redblock")
}

#[tokio::main]
async fn main() {
    let stats = Arc::new(Mutex::new(load_stats()));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(handle_root))
        .route(
            "/test",
            get({
                let stats = Arc::clone(&stats);
                move |q| handle_test(q, Arc::clone(&stats))
            }),
        )
        .route("/stats", get(handle_stats))
        .layer(cors);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("failed to bind port 8080");

    println!("Redblock microservice is now listening on port 8080 :3");

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            signal::ctrl_c()
                .await
                .expect("ctrl+c failed");
        })
        .await
        .unwrap();
}
