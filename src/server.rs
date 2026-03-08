// ============================================================
//  server.rs — struktury danych i logika pobierania serwerów
// ============================================================

use anyhow::Result;
use egui;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayZServer {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub players: u32,
    pub max_players: u32,
    pub ping_ms: Option<u32>,
    pub server_type: ServerType,
    pub country: String,
    pub is_hardcore: bool,
    pub has_battleye: bool,
    pub time_of_day: f32,
    pub mods: Vec<String>,
    pub mods_installed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServerType {
    Official,
    Community,
    Modded,
}

impl ServerType {
    pub fn label(&self) -> &str {
        match self {
            ServerType::Official => "Oficjalny",
            ServerType::Community => "Community",
            ServerType::Modded => "Modded",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServerFilters {
    pub search_query: String,
    pub hide_full: bool,
    pub only_compatible_mods: bool,
    pub max_ping: Option<u32>,
    pub server_type: Option<ServerType>,
    pub map_filter: String,
}

impl Default for ServerFilters {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            hide_full: false,
            only_compatible_mods: false,
            max_ping: None,
            server_type: None,
            map_filter: String::new(),
        }
    }
}

impl DayZServer {
    pub fn matches_filters(&self, filters: &ServerFilters) -> bool {
        if !filters.search_query.is_empty() {
            let q = filters.search_query.to_lowercase();
            if !self.name.to_lowercase().contains(&q)
                && !self.country.to_lowercase().contains(&q)
                && !self.ip.contains(&q)
            {
                return false;
            }
        }
        if filters.hide_full && self.players >= self.max_players {
            return false;
        }
        if filters.only_compatible_mods && !self.mods_installed {
            return false;
        }
        if let Some(max_ping) = filters.max_ping {
            if let Some(ping) = self.ping_ms {
                if ping > max_ping {
                    return false;
                }
            }
        }
        if let Some(ref st) = filters.server_type {
            if &self.server_type != st {
                return false;
            }
        }
        if !filters.map_filter.is_empty() {
            if !self
                .country
                .to_lowercase()
                .contains(&filters.map_filter.to_lowercase())
            {
                return false;
            }
        }
        true
    }

    pub fn ping_color(&self) -> egui::Color32 {
        match self.ping_ms {
            Some(p) if p < 60 => egui::Color32::from_rgb(80, 200, 90),
            Some(p) if p < 120 => egui::Color32::from_rgb(220, 180, 50),
            Some(_) => egui::Color32::from_rgb(220, 60, 40),
            None => egui::Color32::GRAY,
        }
    }

    pub fn fill_ratio(&self) -> f32 {
        if self.max_players == 0 {
            return 0.0;
        }
        self.players as f32 / self.max_players as f32
    }
}

// ----------------------------------------------------------
// Helpers to safely extract values from serde_json::Value
// ----------------------------------------------------------

fn json_str<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)?.as_str()
}

fn json_u64(v: &Value, key: &str) -> Option<u64> {
    v.get(key)?.as_u64()
}

fn json_bool(v: &Value, key: &str) -> bool {
    v.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn json_f64(v: &Value, key: &str) -> Option<f64> {
    v.get(key)?.as_f64()
}

// ----------------------------------------------------------
// Fetch from BattleMetrics using raw Value to avoid any
// deserialization mismatch with the actual API response shape
// ----------------------------------------------------------

pub async fn fetch_servers(search: &str) -> Result<Vec<DayZServer>> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let mut url = "https://api.battlemetrics.com/servers\
        ?filter[game]=dayz\
        &filter[status]=online\
        &sort=-players\
        &page[size]=100"
        .to_string();

    if !search.is_empty() {
        // BattleMetrics supports server name search via filter[search]
        let encoded = search
            .chars()
            .flat_map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == ' ' {
                    vec![c]
                } else {
                    vec![]
                }
            })
            .collect::<String>()
            .replace(' ', "%20");
        url.push_str(&format!("&filter[search]={}", encoded));
    }

    let text = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    // Parse as raw JSON Value — won't fail due to unexpected fields
    let root: Value = serde_json::from_str(&text).map_err(|e| {
        anyhow::anyhow!(
            "JSON parse error: {e}\nBody: {}",
            &text[..text.len().min(500)]
        )
    })?;

    let data = root
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| anyhow::anyhow!("Missing 'data' array in response"))?;

    let mut servers = Vec::new();

    for entry in data {
        let attrs = match entry.get("attributes") {
            Some(a) => a,
            None => continue,
        };

        let ip = match json_str(attrs, "ip") {
            Some(s) => s.to_string(),
            None => continue,
        };
        let port = match json_u64(attrs, "port") {
            Some(p) => p as u16,
            None => continue,
        };
        let name = json_str(attrs, "name").unwrap_or("Unknown").to_string();
        let players = json_u64(attrs, "players").unwrap_or(0) as u32;
        let max_players = json_u64(attrs, "maxPlayers").unwrap_or(0) as u32;

        // details is a nested object — use empty object as fallback
        let empty = Value::Object(Default::default());
        let details = attrs.get("details").unwrap_or(&empty);

        // country
        let country = json_str(attrs, "country").unwrap_or("Unknown").to_string();

        // Server type
        let is_official = json_bool(details, "official");
        let is_modded = json_bool(details, "modded");
        let server_type = if is_official {
            ServerType::Official
        } else if is_modded {
            ServerType::Modded
        } else {
            ServerType::Community
        };

        // Other detail fields
        let is_hardcore = json_bool(details, "hardcore");
        let has_battleye = json_bool(details, "battleye") || json_bool(attrs, "battleye");
        let time_of_day = json_f64(details, "time").unwrap_or(12.0) as f32;

        // Mods — array of objects with "name" or "id"
        let mods: Vec<String> = details
            .get("mods")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        json_str(m, "name")
                            .or_else(|| json_str(m, "id"))
                            .map(|s| s.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default();

        servers.push(DayZServer {
            name,
            ip: format!("{}:{}", ip, port),
            port,
            players,
            max_players,
            ping_ms: None,
            server_type,
            country,
            is_hardcore,
            has_battleye,
            time_of_day,
            mods,
            mods_installed: false,
        });
    }

    log::info!("BattleMetrics: parsed {} servers", servers.len());
    Ok(servers)
}

// ----------------------------------------------------------
// UDP A2S_INFO ping — measures real latency to each server
// Sends the standard Source Engine query packet and times the response
// ----------------------------------------------------------

/// Pings a single server via UDP A2S_INFO, returns RTT in ms or None on timeout
async fn ping_server(ip: &str, port: u16) -> Option<u32> {
    use tokio::net::UdpSocket;
    use tokio::time::{timeout, Duration};

    // A2S_INFO request packet
    let payload: &[u8] = &[
        0xFF, 0xFF, 0xFF, 0xFF, // header
        0x54, // A2S_INFO
        b'S', b'o', b'u', b'r', b'c', b'e', b' ', b'E', b'n', b'g', b'i', b'n', b'e', b' ', b'Q',
        b'u', b'e', b'r', b'y', 0x00,
    ];

    let addr = format!("{}:{}", ip, port);
    let sock = UdpSocket::bind("0.0.0.0:0").await.ok()?;
    sock.connect(&addr).await.ok()?;

    let t0 = std::time::Instant::now();
    sock.send(payload).await.ok()?;

    let mut buf = [0u8; 1400];
    timeout(Duration::from_millis(1500), sock.recv(&mut buf))
        .await
        .ok()?
        .ok()?;

    Some(t0.elapsed().as_millis() as u32)
}

/// Pings all servers concurrently, fills in ping_ms in place
pub async fn ping_servers(servers: &mut Vec<DayZServer>) {
    use futures::future::join_all;

    let tasks: Vec<_> = servers
        .iter()
        .map(|s| {
            // ip field is "host:port" — split it back out
            let parts: Vec<&str> = s.ip.splitn(2, ':').collect();
            let host = parts.first().copied().unwrap_or("").to_string();
            let port = s.port;
            async move { ping_server(&host, port).await }
        })
        .collect();

    let results = join_all(tasks).await;
    for (server, ping) in servers.iter_mut().zip(results) {
        server.ping_ms = ping;
    }
}
