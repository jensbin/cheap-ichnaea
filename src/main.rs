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
use async_std::task;

#[derive(Serialize, Deserialize, Clone)]
struct Location {
    lat: f64,
    lng: f64,
}

#[derive(Serialize, Deserialize, Clone)]
struct Response {
    location: Location,
    accuracy: f64,
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
    cc: String,
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

    fn get(&mut self, key: &str) -> Option<CacheEntry> {
        // Check if the key exists in the cache and is still valid
        if let Some(entry) = self.cache.get(key) {
            let now = SystemTime::now();
            if now.duration_since(entry.timestamp).unwrap() < self.ttl {
                // Cache hit and valid
                return Some(entry.clone());
            } else {
                // Cache hit but expired, remove it
                self.cache.remove(key);
            }
        }
        // Cache miss
        None
    }

    fn insert(&mut self, key: String, value: CacheEntry) {
        self.cache.insert(key, value);
    }
}

// http://ip-api.com/json/?fields=status,message,country,countryCode,lat,lon
// {"status":"success","country":"Switzerland","countryCode":"CH","lat":47.000,"lon":8.000}
fn extract_ipapi(json_str: &str) -> Option<(String, f64, f64)> {
    let json: Value = serde_json::from_str(json_str).ok()?;
    let country = json["countryCode"].as_str().unwrap_or("").to_string();
    let lat: f64 = json["lat"].as_f64().unwrap_or(0.0);
    let lon: f64 = json["lon"].as_f64().unwrap_or(0.0);
    Some((country, lat, lon))
}

// Function to fetch URL with retries and exponential backoff
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
                task::sleep(std::time::Duration::from_secs(wait_interval)).await;
                wait_interval *= 2; // Exponential backoff
            }
        }
    }
}

pub async fn fetch_url(url: &str) -> Result<String, reqwest::Error> {
    debug!("Fetching URL: {}", url);
    let response = reqwest::get(url).await?;
    // info!("Successfully fetched URL");
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
async fn geolocate(_data: Option<web::Json<PostData>>, cache: web::Data<std::sync::Mutex<CountryCache>>) -> impl Responder {

    let mut cache = cache.lock().unwrap(); 

    let (latitude, longitude) = match cache.get("location") {
        Some(cached_entry) => {
            info!("Cache hit for location ({})", cached_entry.cc);
            (cached_entry.coordinates.0, cached_entry.coordinates.1)
        },
        None => {
            info!("Cache miss for location");

            let location_result = fetch_url_with_retry("http://ip-api.com/json/?fields=status,message,country,countryCode,lat,lon", 6, 30).await;

            let (location, latitude, longitude) = match location_result {
                Ok(response) => extract_ipapi(&response).unwrap_or_else(|| ("Unknown".to_string(), 0.0, 0.0)),
                Err(_) => {
                    error!("Failed to fetch location after retries");
                    ("Unknown".to_string(), 0.0, 0.0)
                }
            };
            info!("Found coordinates for {}: ({}, {})", location, latitude, longitude);

            if location.len() == 2 && latitude != 0.0 && longitude != 0.0 {
                let cache_entry = CacheEntry {
                    cc: location.clone(),
                    coordinates: (latitude, longitude),
                    timestamp: SystemTime::now(),
                };
                cache.insert("location".to_string(), cache_entry);
            }

            (latitude, longitude)
        }
    };

    if latitude == 0.0 && longitude == 0.0 {
        // Return 404 with error message
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
        return HttpResponse::NotFound().json(error_response);
    } else {
        let response = Response {
            location: Location { lat: latitude, lng: longitude },
            accuracy: 600000.0,
            fallback: "ipf".to_string(),
        };
        return HttpResponse::Ok().json(response);
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

    let cache = web::Data::new(std::sync::Mutex::new(CountryCache::new(args.ttl_cache))); 

    HttpServer::new(move || {
        App::new()
            .app_data(cache.clone())
            .service(web::resource("/v1/geolocate")
                .route(web::post().to(geolocate)))
    }).workers(2)
    .bind(&format!("127.0.0.1:{}", args.port))?
    .run()
    .await
}
