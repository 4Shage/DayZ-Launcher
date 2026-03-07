# 🎮 DayZ Launcher — Rust / egui

Niestandardowy launcher dla DayZ napisany w języku Rust z interfejsem egui.

## 📁 Struktura projektu

```
dayz-launcher/
├── Cargo.toml          # Zależności i konfiguracja projektu
└── src/
    ├── main.rs         # Punkt wejścia, konfiguracja okna
    ├── app.rs          # Główna logika UI (najważniejszy plik)
    ├── server.rs       # Struktury danych serwerów + filtrowanie
    ├── profile.rs      # Profil gracza, ustawienia, zapis na dysk
    ├── updater.rs      # Sprawdzanie i pobieranie aktualizacji
    └── launcher.rs     # Uruchamianie gry (Steam / bezpośrednio)
```

## ⚙️ Wymagania

- **Rust** (najnowsza stabilna wersja): https://rustup.rs
- **Windows 10/11** (działa też na Linux z Wine)
- Na Windows: Visual C++ Build Tools lub Visual Studio

## 🚀 Uruchomienie (tryb deweloperski)

```bash
# Sklonuj lub rozpakuj projekt
cd dayz-launcher

# Uruchom w trybie debug (wolniej buduje, szybciej kompiluje)
cargo run

# Lub w trybie release (szybsza aplikacja, dłuższa kompilacja)
cargo run --release
```

> Pierwsze budowanie zajmie ~2-5 minut (pobieranie i kompilacja zależności).
> Kolejne będą znacznie szybsze dzięki cache.

## 📦 Budowanie finalnego .exe

```bash
cargo build --release
```

Plik wykonywalny znajdziesz w: `target/release/dayz-launcher.exe`

## 🎨 Modyfikacja motywu

Kolory są zdefiniowane jako stałe w `src/app.rs`:
```rust
const RUST_COLOR: Color32 = Color32::from_rgb(192, 58, 0);   // Główny akcent
const RUST_DIM:   Color32 = Color32::from_rgb(80, 25, 5);    // Przyciemniony akcent
const PANEL_BG:   Color32 = Color32::from_rgb(22, 22, 20);   // Tło paneli
const DIM_COLOR:  Color32 = Color32::from_rgb(130, 125, 110); // Tekst drugorzędny
```

## 📝 Następne kroki (TODO)

- [x] Podłączyć prawdziwe API serwerów DayZ
- [ ] Dodać zarządzanie modami (Steam Workshop)
- [ ] Dodać ikonę aplikacji (.ico na Windows)
- [ ] Implementacja prawdziwego pobierania aktualizacji przez Steam API
- [ ] Dodać serwery ulubione z gwiazdką
- [ ] Tray icon (minimalizacja do zasobnika)
- [ ] Historia ostatnio odwiedzonych serwerów

## 📚 Przydatne zasoby

- [egui docs](https://docs.rs/egui/latest/egui/) — dokumentacja frameworka UI
- [eframe docs](https://docs.rs/eframe/latest/eframe/) — wrapper okna
- [The Rust Book PL](https://doc.rust-lang.org/book/) — nauka Rust
- [DayZ AppID: 221100](https://store.steampowered.com/app/221100/)
