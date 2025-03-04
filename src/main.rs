use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use clap::{self, Parser};
use reqwest;
use std::io::Write;
use serde_json::{self, Value};
use log::{info, warn, debug, error};
use env_logger::Env;
use std::time::{Duration, SystemTime};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::time;

#[derive(Serialize, Deserialize, Clone)]
struct Location {
    lat: f64,
    lng: f64,
}

#[derive(Serialize, Deserialize, Clone)]
struct LocationResponse {
    location: Location,
    accuracy: f64,
    fallback: String,
}

#[derive(Serialize, Deserialize)]
struct CountryResponse {
    country_code: String,
    country_name: String,
    fallback: String,
}

#[derive(Serialize, Deserialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Serialize, Deserialize)]
struct ErrorDetail {
    errors: Vec<ErrorInfo>,
    code: u16,
    message: String,
}

#[derive(Serialize, Deserialize)]
struct ErrorInfo {
    domain: String,
    reason: String,
    message: String,
}

// Cache entry struct
#[derive(Debug, Clone)]
struct CacheEntry {
    country_code: String,
    country_name: String,
    coordinates: (f64, f64), // (latitude, longitude)
    timestamp: SystemTime,
}

// Implement a simple in-memory cache with a configurable TTL
#[derive(Debug)]
struct CountryCache {
    cache: HashMap<String, CacheEntry>,
    ttl: Duration,
}

impl CountryCache {
    fn new(ttl_seconds: u64) -> Self {
        CountryCache {
            cache: HashMap::new(),
            ttl: Duration::from_secs(ttl_seconds),
        }
    }

    fn get(&self, key: &str) -> Option<CacheEntry> {
        // Check if the key exists in the cache and is still valid
        if let Some(entry) = self.cache.get(key) {
            let now = SystemTime::now();
            if now.duration_since(entry.timestamp).unwrap() < self.ttl {
                // Cache hit and valid
                return Some(entry.clone());
            } 
        }
        // Cache miss or expired
        None
    }

    fn insert(&mut self, key: String, value: CacheEntry) {
        self.cache.insert(key, value);
    }

    fn clear_expired(&mut self) {
        let now = SystemTime::now();
        self.cache.retain(|_, v| now.duration_since(v.timestamp).unwrap() < self.ttl);
    }
}

async fn fetch_location() -> Option<(String, String, f64, f64)> {
    let location_result = fetch_url_with_retry("http://ip-api.com/json/?fields=status,message,country,countryCode,lat,lon", 6, 30).await;

    match location_result {
        Ok(response) => extract_ipapi(&response),
        Err(_) => {
            error!("Failed to fetch location after retries");
            None
        }
    }
}

async fn update_location_loop(cache: Arc<RwLock<CountryCache>>) {
    loop {
        let ttl = cache.read().unwrap().ttl;
        if let Some((country_code, country_name, latitude, longitude)) = fetch_location().await {
            if country_code.len() == 2 && latitude != 0.0 && longitude != 0.0 {
                let cache_entry = CacheEntry {
                    country_code: country_code.clone(),
                    country_name: country_name.clone(),
                    coordinates: (latitude, longitude),
                    timestamp: SystemTime::now(),
                };
                cache.write().unwrap().insert("location".to_string(), cache_entry);
                info!("Updated location cache: {} ({}, {})", country_code, latitude, longitude);
            }
        }
        time::sleep(ttl).await;

        // Clear expired entries after each update
        cache.write().unwrap().clear_expired();
    }
}

fn extract_ipapi(json_str: &str) -> Option<(String, String, f64, f64)> {
    let json: Value = serde_json::from_str(json_str).ok()?;

    let country_code = json["countryCode"].as_str().map_or_else(String::new, String::from);
    let country_name = json["country"].as_str().map_or_else(String::new, String::from);
    let lat: f64 = json["lat"].as_f64().unwrap_or(0.0);
    let lon: f64 = json["lon"].as_f64().unwrap_or(0.0);
    Some((country_code, country_name, lat, lon))
}


async fn fetch_url_with_retry(url: &str, max_retries: u32, retry_interval: u64) -> Result<String, reqwest::Error> {
    let mut retries = 0;
    let mut wait_interval = retry_interval;

    loop {
        match fetch_url(url).await {
            Ok(response) => {
                info!("Successfully fetched URL: {}", url);
                return Ok(response);
            }
            Err(err) => {
                retries += 1;
                if retries > max_retries {
                    error!("Max retries exceeded. Failed to fetch URL: {}", url);
                    return Err(err);
                }

                warn!("Failed to fetch URL (attempt {}/{}): {}. Retrying in {} seconds...", retries, max_retries, err, wait_interval);
                time::sleep(Duration::from_secs(wait_interval)).await;
                wait_interval = (wait_interval as f64 * 1.3) as u64;
            }
        }
    }
}

pub async fn fetch_url(url: &str) -> Result<String, reqwest::Error> {
    debug!("Fetching URL: {}", url);
    let response = reqwest::get(url).await?;
    Ok(response.text().await?)
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// The port number to listen on
    #[arg(short, long, default_value_t = 8080)]
    pub port: u16,

    /// Cache TTL in seconds
    #[arg(short, long, default_value_t = 30 * 60)]
    pub ttl_cache: u64,
}

// Define a struct to hold the POST request data
#[allow(dead_code)]
#[derive(Deserialize)]
#[derive(Debug)]
struct PostData {
    carrier: Option<String>,
    consider_ip: Option<bool>,
    home_mobile_country_code: Option<u32>,
    home_mobile_network_code: Option<u32>,
    bluetooth_beacons: Option<Vec<BluetoothBeacons>>,
    cell_towers: Option<Vec<CellTowers>>,
    wifi_access_points: Option<Vec<WifiAccessPoints>>,
    fallbacks: Option<Vec<Fallbacks>>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[derive(Debug)]
struct Fallbacks {
    lacf: bool,
    ipf: bool,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[derive(Debug)]
struct BluetoothBeacons {
    mac_address: String,
    age: u32,
    name: String,
    signal_strength: i32,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[derive(Debug)]
struct CellTowers {
    radio_type: String,
    mobile_country_code: u32,
    mobile_network_code: u32,
    location_area_code: u32,
    cell_id: u32,
    age: u32,
    psc: u32,
    signal_strength: i32,
    timing_advance: u32,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[derive(Debug)]
struct WifiAccessPoints {
    mac_address: String,
    age: u32,
    channel: u32,
    frequency: u32,
    signal_strength: i32,
    signal_to_noise_ratio: u32,
}

// Define the get_location function to handle POST requests
async fn geolocate(cache: web::Data<Arc<RwLock<CountryCache>>>) -> impl Responder {
    let cache = cache.read().unwrap();

    match cache.get("location") {
        Some(cached_entry) => {
            info!("Cache hit for location ({})", cached_entry.country_code);
            let response = LocationResponse {
                location: Location { lat: cached_entry.coordinates.0, lng: cached_entry.coordinates.1 },
                accuracy: 600000.0,
                fallback: "ipf".to_string(),
            };
            HttpResponse::Ok().json(response)
        },
        None => {
            info!("Location unknown");
            let error_response = ErrorResponse {
                error: ErrorDetail {
                    errors: vec![ErrorInfo {
                        domain: "geolocation".to_string(),
                        reason: "notFound".to_string(),
                        message: "Not found".to_string(),
                    }],
                    code: 404,
                    message: "Not found".to_string(),
                }
            };
            HttpResponse::NotFound().json(error_response)

        }
    }
}

async fn country(cache: web::Data<Arc<RwLock<CountryCache>>>) -> impl Responder {
    let cache = cache.read().unwrap();

    match cache.get("location") {
        Some(cached_entry) => {
            info!("Cache hit for country ({})", cached_entry.country_code);
            let response = CountryResponse {
                country_code: cached_entry.country_code,
                country_name: cached_entry.country_name,
                fallback: "ipf".to_string(),
            };
            HttpResponse::Ok().json(response)
        }
        None => {
            info!("Country unknown");

            let error_response = ErrorResponse {
                error: ErrorDetail {
                   errors: vec![ErrorInfo {
                       domain: "geolocation".to_string(),
                       reason: "notFound".to_string(),
                        message: "Not found".to_string(),
                    }],
                    code: 404,
                    message: "Not found".to_string(),
               }
           };
           HttpResponse::NotFound().json(error_response)
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    match std::env::var("RUST_LOG_STYLE") {
        Ok(s) if s == "SYSTEMD" => env_logger::builder()
            .format(|buf, record| {
                writeln!(
                    buf,
                    "<{}>{}: {}",
                    match record.level() {
                        log::Level::Error => 3,
                        log::Level::Warn => 4,
                        log::Level::Info => 6,
                        log::Level::Debug => 7,
                        log::Level::Trace => 7,
                    },
                    record.target(),
                    record.args()
                )
            })
            .init(),
        _ => env_logger::Builder::from_env(Env::default().default_filter_or("info")).format_timestamp(None).init(),
    };
    let args = Args::parse();

    let cache = Arc::new(RwLock::new(CountryCache::new(args.ttl_cache)));
    let cloned_cache = cache.clone();


    tokio::spawn(async move {
        update_location_loop(cloned_cache).await;
    });


    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(cache.clone()))
            .service(web::resource("/v1/geolocate").route(web::post().to(geolocate)))
            .service(web::resource("/v1/country").route(web::post().to(country)))
    })
    .workers(3)
    .bind(("localhost", args.port))?
    .run()
    .await
}
