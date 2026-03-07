// ============================================================
// modules/theme.rs — Motyw wizualny launchera
// ============================================================
// Tutaj trzymamy wszystkie kolory i style.
// Dzięki temu zmiana wyglądu to edycja jednego pliku.

use egui::{Color32, FontId, Rounding, Stroke, Style, Visuals};

// --- Paleta kolorów ---
// Color32::from_rgb(R, G, B) — kolor z wartości 0–255

pub struct DayZTheme;

impl DayZTheme {
    // Tło aplikacji — bardzo ciemny szary
    pub const BG_PRIMARY: Color32 = Color32::from_rgb(18, 18, 16);
    // Tło paneli — ciemniejszy szary
    pub const BG_PANEL: Color32 = Color32::from_rgb(26, 26, 22);
    // Tło elementów (np. wiersze tabeli)
    pub const BG_ITEM: Color32 = Color32::from_rgb(32, 32, 28);
    // Tło zaznaczonego elementu
    pub const BG_SELECTED: Color32 = Color32::from_rgb(80, 25, 5);
    // Tło przycisku hover
    pub const BG_HOVER: Color32 = Color32::from_rgb(50, 20, 5);

    // Kolor akcentu — rdzawa czerwień (charakterystyczna dla DayZ)
    pub const ACCENT: Color32 = Color32::from_rgb(192, 58, 0);
    pub const ACCENT_BRIGHT: Color32 = Color32::from_rgb(255, 69, 0);
    pub const ACCENT_DIM: Color32 = Color32::from_rgb(120, 36, 0);

    // Kolory tekstu
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(200, 191, 168);
    pub const TEXT_DIM: Color32 = Color32::from_rgb(120, 117, 104);
    pub const TEXT_WHITE: Color32 = Color32::from_rgb(240, 235, 225);

    // Kolory statusów
    pub const GREEN: Color32 = Color32::from_rgb(92, 138, 94);
    pub const YELLOW: Color32 = Color32::from_rgb(200, 160, 0);
    pub const RED: Color32 = Color32::from_rgb(192, 58, 0);

    // Kolor obramowań
    pub const BORDER: Color32 = Color32::from_rgb(60, 40, 20);

    /// Stosuje motyw do całego egui
    pub fn apply(ctx: &egui::Context) {
        let mut style = Style::default();
        let mut visuals = Visuals::dark();

        // Tło głównego okna
        visuals.panel_fill = Self::BG_PRIMARY;
        visuals.window_fill = Self::BG_PANEL;

        // Kolor zaznaczenia (selection)
        visuals.selection.bg_fill = Self::BG_SELECTED;
        visuals.selection.stroke = Stroke::new(1.0, Self::ACCENT);

        // Przyciski — domyślny wygląd
        visuals.widgets.inactive.bg_fill = Self::ACCENT_DIM;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, Self::TEXT_PRIMARY);
        visuals.widgets.inactive.rounding = Rounding::same(2.0);

        // Przyciski — hover (myszka nad przyciskiem)
        visuals.widgets.hovered.bg_fill = Self::ACCENT;
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, Self::TEXT_WHITE);

        // Przyciski — kliknięcie
        visuals.widgets.active.bg_fill = Self::ACCENT_BRIGHT;
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);

        // Obramowania
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Self::BORDER);

        style.visuals = visuals;

        // Odstępy między elementami
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);

        ctx.set_style(style);
    }

    /// Rysuje nagłówek sekcji (np. "SERWERY", "PROFIL")
    pub fn section_header(ui: &mut egui::Ui, label: &str) {
        ui.add_space(4.0);
        // Kolorowy tekst nagłówka
        ui.colored_label(Self::ACCENT, format!("▶ {}", label));
        // Pozioma linia pod nagłówkiem
        ui.separator();
        ui.add_space(2.0);
    }

    /// Styl czcionki dla dużych tytułów
    pub fn font_title() -> FontId {
        FontId::proportional(22.0)
    }

    /// Styl czcionki dla małych etykiet
    pub fn font_small() -> FontId {
        FontId::monospace(10.0)
    }
}
