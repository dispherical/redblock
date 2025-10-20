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
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::signal;
use tower_http::cors::{Any, CorsLayer};
use ipnet::IpNet;

#[derive(Default, Serialize, Deserialize, Clone)]
struct Stats {
    requests: u64,
    blocks: u64,
    passes: u64,
}

struct AtomicStats {
    requests: AtomicU64,
    blocks: AtomicU64,
    passes: AtomicU64,
}

impl AtomicStats {
    fn new(stats: Stats) -> Self {
        Self {
            requests: AtomicU64::new(stats.requests),
            blocks: AtomicU64::new(stats.blocks),
            passes: AtomicU64::new(stats.passes),
        }
    }

    fn snapshot(&self) -> Stats {
        Stats {
            requests: self.requests.load(Ordering::Relaxed),
            blocks: self.blocks.load(Ordering::Relaxed),
            passes: self.passes.load(Ordering::Relaxed),
        }
    }
}

#[derive(Clone)]
struct Blocklist {
    v4: Arc<Vec<Range4>>,
    v6: Arc<Vec<Range6>>,
}

#[derive(Clone, Copy)]
struct Range4 {
    start: u32,
    end: u32,
}

#[derive(Clone, Copy)]
struct Range6 {
    start: u128,
    end: u128,
}

const STATS_FILE: &str = "redblock-stats.json";

fn ipv4_to_u32(ip: std::net::Ipv4Addr) -> u32 {
    u32::from_be_bytes(ip.octets())
}
fn ipv6_to_u128(ip: std::net::Ipv6Addr) -> u128 {
    u128::from_be_bytes(ip.octets())
}

fn merge_v4(mut ranges: Vec<Range4>) -> Vec<Range4> {
    ranges.sort_by_key(|r| r.start);
    let mut out: Vec<Range4> = Vec::new();
    for r in ranges {
        if let Some(last) = out.last_mut() {
            if r.start <= last.end + 1 {
                last.end = last.end.max(r.end);
                continue;
            }
        }
        out.push(r);
    }
    out
}
fn merge_v6(mut ranges: Vec<Range6>) -> Vec<Range6> {
    ranges.sort_by_key(|r| r.start);
    let mut out: Vec<Range6> = Vec::new();
    for r in ranges {
        if let Some(last) = out.last_mut() {
            if r.start <= last.end + 1 {
                last.end = last.end.max(r.end);
                continue;
            }
        }
        out.push(r);
    }
    out
}

fn contains_v4(ranges: &[Range4], ip: u32) -> bool {
    match ranges.binary_search_by_key(&ip, |r| r.start) {
        Ok(_) => true,
        Err(0) => false,
        Err(i) => ip <= ranges[i - 1].end,
    }
}

fn contains_v6(ranges: &[Range6], ip: u128) -> bool {
    match ranges.binary_search_by_key(&ip, |r| r.start) {
        Ok(_) => true,
        Err(0) => false,
        Err(i) => ip <= ranges[i - 1].end,
    }
}

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

fn load_blocklist(path: &str) -> Blocklist {
    let data = fs::read_to_string(path).unwrap_or_default();
    let mut v4 = Vec::new();
    let mut v6 = Vec::new();

    for line in data.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if let Ok(net) = line.parse::<IpNet>() {
            match net {
                IpNet::V4(n) => {
                    let (s, e) = (ipv4_to_u32(n.network()), ipv4_to_u32(n.broadcast()));
                    v4.push(Range4 { start: s, end: e });
                }
                IpNet::V6(n) => {
                    let (s, e) = (ipv6_to_u128(n.network()), ipv6_to_u128(n.broadcast()));
                    v6.push(Range6 { start: s, end: e });
                }
            }
        }
    }

    Blocklist {
        v4: Arc::new(merge_v4(v4)),
        v6: Arc::new(merge_v6(v6)),
    }
}

async fn handle_test(
    Query(params): Query<HashMap<String, String>>,
    stats: Arc<AtomicStats>,
    blocklist: Arc<Blocklist>,
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

    let ip_parsed: IpAddr = match ip.parse() {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "invalid ip" })),
            )
        }
    };

    let blocked = match ip_parsed {
        IpAddr::V4(v4addr) => contains_v4(&blocklist.v4, ipv4_to_u32(v4addr)),
        IpAddr::V6(v6addr) => contains_v6(&blocklist.v6, ipv6_to_u128(v6addr)),
    };

    stats.requests.fetch_add(1, Ordering::Relaxed);
    if blocked {
        stats.blocks.fetch_add(1, Ordering::Relaxed);
    } else {
        stats.passes.fetch_add(1, Ordering::Relaxed);
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "blocked": blocked })),
    )
}

async fn handle_stats(stats: Arc<AtomicStats>) -> impl IntoResponse {
    let s = stats.snapshot();
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
    let stats = Arc::new(AtomicStats::new(load_stats()));
    let blocklist = Arc::new(load_blocklist("/root/list.txt"));

    {
        let stats = Arc::clone(&stats);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                save_stats(&stats.snapshot());
            }
        });
    }

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
                let blocklist = Arc::clone(&blocklist);
                move |q| handle_test(q, Arc::clone(&stats), Arc::clone(&blocklist))
            }),
        )
        .route("/stats", get({
            let stats = Arc::clone(&stats);
            move || handle_stats(Arc::clone(&stats))
        }))
        .layer(cors);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("failed to bind port 8080");

    println!("Redblock microservice is now listening on port 8080 :3");

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            signal::ctrl_c().await.expect("ctrl+c failed");
        })
        .await
        .unwrap();
}
