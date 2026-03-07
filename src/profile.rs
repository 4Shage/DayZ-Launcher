// ============================================================
//  profile.rs — profil gracza i ustawienia launchera
//  Dane są zapisywane do pliku JSON w folderze konfiguracji
// ============================================================

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ----------------------------------------------------------
// Profil gracza
// ----------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerProfile {
    /// Nazwa wyświetlana w launcherze
    pub display_name: String,
    /// Ścieżka do katalogu instalacji DayZ
    pub game_path: String,
    /// Ostatnio użyty serwer (IP:port)
    pub last_server: Option<String>,
    /// Lista ulubionych serwerów (IP:port)
    pub favorite_servers: Vec<String>,
    /// Ustawienia gry
    pub game_settings: GameSettings,
    /// Ustawienia launchera (wygląd, zachowanie)
    pub launcher_settings: LauncherSettings,
    /// Łączny czas spędzony w launcherze (sekundy) — opcjonalne
    pub total_launches: u32,
}

impl Default for PlayerProfile {
    fn default() -> Self {
        Self {
            display_name: "Ocalały".into(),
            // Domyślna ścieżka Steam na Windows
            game_path: default_game_path(),
            last_server: None,
            favorite_servers: vec![],
            game_settings: GameSettings::default(),
            launcher_settings: LauncherSettings::default(),
            total_launches: 0,
        }
    }
}

// ----------------------------------------------------------
// Ustawienia samej gry (parametry wiersza poleceń DayZ)
// ----------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSettings {
    /// Ilość wątków używanych przez grę (0 = auto)
    pub cpu_count: u8,
    /// Rozmiar pliku paginacji (0 = domyślny)
    pub file_patching: bool,
    /// Pokaż konsolę diagnostyczną przy starcie
    pub show_script_errors: bool,
    /// Dodatkowe parametry uruchomienia (zaawansowane)
    pub extra_launch_params: String,
    /// Preferowany język gry
    pub language: GameLanguage,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            cpu_count: 0,
            file_patching: false,
            show_script_errors: false,
            extra_launch_params: String::new(),
            language: GameLanguage::Polish,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GameLanguage {
    Polish,
    English,
    German,
    Czech,
}

impl GameLanguage {
    pub fn all() -> &'static [GameLanguage] {
        &[
            GameLanguage::Polish,
            GameLanguage::English,
            GameLanguage::German,
            GameLanguage::Czech,
        ]
    }

    pub fn label(&self) -> &str {
        match self {
            GameLanguage::Polish => "Polski",
            GameLanguage::English => "English",
            GameLanguage::German => "Deutsch",
            GameLanguage::Czech => "Čeština",
        }
    }

    /// Kod języka używany jako parametr uruchomienia
    pub fn launch_code(&self) -> &str {
        match self {
            GameLanguage::Polish => "Polish",
            GameLanguage::English => "English",
            GameLanguage::German => "German",
            GameLanguage::Czech => "Czech",
        }
    }
}

// ----------------------------------------------------------
// Ustawienia launchera
// ----------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherSettings {
    /// Zamknij launcher po uruchomieniu gry
    pub close_on_launch: bool,
    /// Automatycznie sprawdzaj aktualizacje przy starcie
    pub auto_check_updates: bool,
    /// Minimalizuj do traya zamiast zamykać
    pub minimize_to_tray: bool,
    /// Styl kolorystyczny — "dark" lub "light"
    pub theme: ThemeChoice,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            close_on_launch: false,
            auto_check_updates: true,
            minimize_to_tray: false,
            theme: ThemeChoice::Dark,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ThemeChoice {
    Dark,
    Light,
}

// ----------------------------------------------------------
// Zapis i odczyt profilu z dysku
// ----------------------------------------------------------
impl PlayerProfile {
    /// Ścieżka do pliku konfiguracyjnego
    /// Na Windows: %APPDATA%\dayz-launcher\profile.json
    /// Na Linux:   ~/.config/dayz-launcher/profile.json
    pub fn config_path() -> PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join("dayz-launcher").join("profile.json")
    }

    /// Wczytaj profil z pliku, lub utwórz domyślny
    pub fn load() -> Self {
        let path = Self::config_path();

        // Próbuj wczytać plik; jeśli nie istnieje lub jest uszkodzony — użyj domyślnego
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                log::warn!("Nie można wczytać profilu ({e}), używam domyślnego");
                Self::default()
            }),
            Err(_) => {
                log::info!("Brak pliku profilu, tworzę domyślny");
                Self::default()
            }
        }
    }

    /// Zapisz profil do pliku JSON
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();

        // Utwórz folder jeśli nie istnieje
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("Nie można utworzyć folderu konfiguracji")?;
        }

        // Serializuj do ładnie sformatowanego JSON
        let json = serde_json::to_string_pretty(self).context("Błąd serializacji profilu")?;

        std::fs::write(&path, json)
            .context(format!("Nie można zapisać pliku: {}", path.display()))?;

        log::info!("Profil zapisany: {}", path.display());
        Ok(())
    }

    /// Sprawdź czy ścieżka do gry istnieje i zawiera dayz_x64.exe
    pub fn is_game_path_valid(&self) -> bool {
        let exe = PathBuf::from(&self.game_path).join("DayZ_x64.exe");
        exe.exists()
    }

    /// Dodaj serwer do ulubionych (bez duplikatów)
    #[allow(dead_code)]
    pub fn toggle_favorite(&mut self, server_ip: &str) {
        let ip = server_ip.to_string();
        if let Some(pos) = self.favorite_servers.iter().position(|s| s == &ip) {
            self.favorite_servers.remove(pos); // Usuń jeśli już jest
        } else {
            self.favorite_servers.push(ip); // Dodaj jeśli brak
        }
    }

    #[allow(dead_code)]
    pub fn is_favorite(&self, server_ip: &str) -> bool {
        self.favorite_servers.iter().any(|s| s == server_ip)
    }

    /// Buduje string parametrów uruchomienia DayZ
    pub fn build_launch_args(&self, server_ip: &str, server_port: u16) -> Vec<String> {
        let mut args = vec![
            format!("-connect={}", server_ip),
            format!("-port={}", server_port),
            format!("-language={}", self.game_settings.language.launch_code()),
        ];

        if self.game_settings.file_patching {
            args.push("-filePatching".into());
        }
        if self.game_settings.show_script_errors {
            args.push("-showScriptErrors".into());
        }
        if self.game_settings.cpu_count > 0 {
            args.push(format!("-cpuCount={}", self.game_settings.cpu_count));
        }
        if !self.game_settings.extra_launch_params.is_empty() {
            // Dziel parametry po spacjach i dodaj każdy oddzielnie
            for param in self.game_settings.extra_launch_params.split_whitespace() {
                args.push(param.to_string());
            }
        }

        args
    }
}

/// Domyślna ścieżka instalacji DayZ
fn default_game_path() -> String {
    // Sprawdź typowe ścieżki Steam
    #[cfg(target_os = "windows")]
    {
        let candidates = [
            r"C:\Program Files (x86)\Steam\steamapps\common\DayZ",
            r"C:\Program Files\Steam\steamapps\common\DayZ",
            r"D:\Steam\steamapps\common\DayZ",
        ];
        for path in &candidates {
            if PathBuf::from(path).exists() {
                return path.to_string();
            }
        }
        candidates[0].to_string() // Domyślna wartość
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Linux (Proton/Wine)
        if let Some(home) = dirs::home_dir() {
            return home
                .join(".steam/steam/steamapps/common/DayZ")
                .to_string_lossy()
                .to_string();
        }
        "./DayZ".to_string()
    }
}
