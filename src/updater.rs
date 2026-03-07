// ============================================================
//  updater.rs — sprawdzanie i pobieranie aktualizacji
// ============================================================

use serde::Deserialize;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate { current_version: String },
    UpdateAvailable {
        current_version: String,
        new_version: String,
        size_mb: f32,
        changelog: String,
    },
    Downloading {
        progress: f32,
        speed_mb_s: f32,
        downloaded_mb: f32,
        total_mb: f32,
    },
    ReadyToInstall { version: String },
    Error(String),
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct VersionInfo {
    pub version: String,
    pub build_id: u64,
    pub size_mb: f32,
    pub changelog: String,
    pub download_url: String,
}

pub struct Updater {
    pub status: Arc<Mutex<UpdateStatus>>,
    pub current_version: String,
}

impl Updater {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(UpdateStatus::Idle)),
            current_version: Self::read_installed_version(),
        }
    }

    fn read_installed_version() -> String {
        // TODO: parse steamapps/appmanifest_221100.acf for real buildid
        "1.25.157800".to_string()
    }

    pub fn check_for_updates(&self) {
        let status = Arc::clone(&self.status);
        let current = self.current_version.clone();

        tokio::spawn(async move {
            *status.lock().unwrap() = UpdateStatus::Checking;

            tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

            // TODO: replace with real Steam API / own version endpoint
            let new_version = "1.25.157849".to_string();

            let new_status = if new_version != current {
                UpdateStatus::UpdateAvailable {
                    current_version: current,
                    new_version,
                    size_mb: 1240.0,
                    changelog: concat!(
                        "• Nowe bronie: Mosin-Nagant z lunetą PU\n",
                        "• Poprawki sieciowe — mniejszy desync\n",
                        "• Optymalizacja renderowania drzew (-18% GPU)\n",
                        "• Naprawiono teleportację zombie przez ściany\n",
                        "• System temperatury ciała — hipotermia"
                    )
                    .to_string(),
                }
            } else {
                UpdateStatus::UpToDate {
                    current_version: current,
                }
            };

            *status.lock().unwrap() = new_status;
        });
    }

    pub fn start_download(&self) {
        let status = Arc::clone(&self.status);

        tokio::spawn(async move {
            let total_mb = 1240.0_f32;
            let mut downloaded = 0.0_f32;

            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;

                let chunk = total_mb / 50.0 + rand_f32() * 10.0;
                downloaded = (downloaded + chunk).min(total_mb);
                let speed = 40.0 + rand_f32() * 30.0;

                *status.lock().unwrap() = UpdateStatus::Downloading {
                    progress: downloaded / total_mb,
                    speed_mb_s: speed,
                    downloaded_mb: downloaded,
                    total_mb,
                };

                if downloaded >= total_mb {
                    break;
                }
            }

            *status.lock().unwrap() = UpdateStatus::ReadyToInstall {
                version: "1.25.157849".to_string(),
            };
        });
    }

    pub fn install_update(&self) {
        let status = Arc::clone(&self.status);

        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            *status.lock().unwrap() = UpdateStatus::UpToDate {
                current_version: "1.25.157849".to_string(),
            };
        });
    }

    pub fn get_status(&self) -> UpdateStatus {
        self.status.lock().unwrap().clone()
    }
}

fn rand_f32() -> f32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (ns % 1000) as f32 / 1000.0
}
