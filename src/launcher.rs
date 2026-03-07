// ============================================================
//  launcher.rs — uruchamianie gry przez Steam lub bezpośrednio
// ============================================================

use crate::profile::PlayerProfile;
use crate::server::DayZServer;
use anyhow::{Context, Result};
use std::process::Command;

/// Wynik próby uruchomienia gry
#[derive(Debug)]
#[allow(dead_code)]
pub enum LaunchResult {
    /// Gra uruchomiona pomyślnie
    Success,
    /// Gra uruchomiona, ale launcher mógł się już zamknąć
    SuccessAndClose,
    /// Błąd uruchomienia
    Error(String),
}

// ----------------------------------------------------------
// Uruchamianie przez Steam (rekomendowane)
// DayZ AppID w Steam = 221100
// ----------------------------------------------------------

/// Uruchom grę przez klienta Steam
/// Steam zajmie się weryfikacją plików i modami z warsztatu
pub fn launch_via_steam(server: &DayZServer, _profile: &PlayerProfile) -> Result<()> {
    // Budujemy argumenty connect (dodatkowe parametry po znaku +)
    let connect_arg = format!("-connect={} -port={}", server.ip, server.port);

    // steam://run/221100//<args>/ — protokół Steam do uruchamiania gier
    // Podwójny slash // = brak profilu Steam (użyj aktywnego)
    let steam_url = format!("steam://run/221100//{}/", connect_arg);

    log::info!("Uruchamianie przez Steam: {}", steam_url);

    #[cfg(target_os = "windows")]
    {
        // Na Windows: ShellExecute lub `start` otwiera URL protokołu
        Command::new("cmd")
            .args(["/C", "start", "", &steam_url])
            .spawn()
            .context("Nie można uruchomić Steam. Czy Steam jest zainstalowany?")?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&steam_url)
            .spawn()
            .context("Nie można otworzyć protokołu Steam")?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&steam_url)
            .spawn()
            .context("Nie można otworzyć protokołu Steam")?;
    }

    Ok(())
}

// ----------------------------------------------------------
// Uruchamianie bezpośrednie (omijanie Steam)
// Przydatne gdy Steam nie działa lub dla serwerów modowanych
// ----------------------------------------------------------

/// Uruchom DayZ_x64.exe bezpośrednio z katalogu gry
pub fn launch_direct(server: &DayZServer, profile: &PlayerProfile) -> Result<()> {
    use std::path::PathBuf;

    let game_exe = PathBuf::from(&profile.game_path).join("DayZ_x64.exe");

    // Sprawdź czy plik istnieje
    if !game_exe.exists() {
        return Err(anyhow::anyhow!(
            "Nie znaleziono DayZ_x64.exe w: {}\n\
             Sprawdź ścieżkę w ustawieniach.",
            profile.game_path
        ));
    }

    // Pobierz listę argumentów z profilu
    let args = profile.build_launch_args(&server.ip, server.port);

    log::info!("Uruchamianie: {} {:?}", game_exe.display(), args);

    // Uruchom proces (spawn = nie czekaj na zakończenie)
    Command::new(&game_exe)
        .args(&args)
        // Ustaw katalog roboczy na folder gry
        .current_dir(&profile.game_path)
        .spawn()
        .context(format!("Nie można uruchomić: {}", game_exe.display()))?;

    Ok(())
}

// ----------------------------------------------------------
// Pomocnicze funkcje
// ----------------------------------------------------------

/// Otwórz folder gry w eksploratorze plików
pub fn open_game_folder(path: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    Command::new("explorer")
        .arg(path)
        .spawn()
        .context("Nie można otworzyć Eksploratora")?;

    #[cfg(target_os = "linux")]
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .context("Nie można otworzyć menedżera plików")?;

    #[cfg(target_os = "macos")]
    Command::new("open")
        .arg(path)
        .spawn()
        .context("Nie można otworzyć Findera")?;

    Ok(())
}

/// Otwórz profil Steam w przeglądarce
#[allow(dead_code)]
pub fn open_steam_profile() -> Result<()> {
    let url = "https://store.steampowered.com/app/221100/DayZ/";

    #[cfg(target_os = "windows")]
    Command::new("cmd").args(["/C", "start", "", url]).spawn()?;

    #[cfg(target_os = "linux")]
    Command::new("xdg-open").arg(url).spawn()?;

    #[cfg(target_os = "macos")]
    Command::new("open").arg(url).spawn()?;

    Ok(())
}

/// Sprawdź czy Steam jest zainstalowany i dostępny
pub fn is_steam_available() -> bool {
    #[cfg(target_os = "windows")]
    {
        // Sprawdź typowe lokalizacje steam.exe
        let paths = [
            r"C:\Program Files (x86)\Steam\steam.exe",
            r"C:\Program Files\Steam\steam.exe",
        ];
        paths.iter().any(|p| std::path::Path::new(p).exists())
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Sprawdź czy `steam` jest w PATH
        Command::new("which")
            .arg("steam")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}
