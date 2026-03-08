// ============================================================
//  app.rs — główna aplikacja: stan, logika i renderowanie UI
// ============================================================

use eframe::egui::{self, Color32, FontId, RichText, Rounding, Stroke, Vec2};

use crate::launcher;
use crate::profile::PlayerProfile;
use crate::server::{DayZServer, ServerFilters, ServerType};
use crate::updater::{UpdateStatus, Updater};

#[derive(Debug, Clone, PartialEq)]
enum Tab {
    Servers,
    Profile,
    Settings,
    Updates,
}

pub struct DayZLauncher {
    profile: PlayerProfile,
    servers: Vec<DayZServer>,
    selected_server: Option<usize>,
    filters: ServerFilters,
    updater: Updater,
    active_tab: Tab,
    status_message: String,
    servers_loading: bool,
    runtime: tokio::runtime::Runtime,
    server_rx: Option<std::sync::mpsc::Receiver<Result<Vec<DayZServer>, String>>>,
    ping_rx: Option<std::sync::mpsc::Receiver<Vec<Option<u32>>>>,
    last_fetched_query: String,
    search_debounce: Option<std::time::Instant>,
}

impl DayZLauncher {
    pub fn new(_cc: &eframe::CreationContext) -> Self {
        let runtime = tokio::runtime::Runtime::new().expect("Nie można utworzyć Tokio runtime");
        let profile = PlayerProfile::load();
        let updater = Updater::new();

        if profile.launcher_settings.auto_check_updates {
            updater.check_for_updates();
        }

        let mut app = Self {
            profile,
            servers: Vec::new(),
            selected_server: None,
            filters: ServerFilters::default(),
            updater,
            active_tab: Tab::Servers,
            status_message: "Pobieranie listy serwerów...".into(),
            servers_loading: false,
            runtime,
            server_rx: None,
            ping_rx: None,
            last_fetched_query: String::new(),
            search_debounce: None,
        };

        app.fetch_servers();
        app
    }

    fn fetch_servers(&mut self) {
        let query = self.filters.search_query.clone();
        self.last_fetched_query = query.clone();
        self.servers_loading = true;
        self.selected_server = None;
        self.search_debounce = None;
        if query.is_empty() {
            self.status_message = "⟳ Pobieranie listy serwerów z BattleMetrics...".into();
        } else {
            self.status_message = format!("⟳ Szukanie serwerów: \"{}\"...", query);
        }

        let (tx, rx) = std::sync::mpsc::channel();
        self.server_rx = Some(rx);

        self.runtime.spawn(async move {
            let result = crate::server::fetch_servers(&query)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
    }

    fn poll_server_results(&mut self) {
        let received = if let Some(rx) = &self.server_rx {
            rx.try_recv().ok()
        } else {
            None
        };

        // Poll ping results
        if let Some(rx) = &self.ping_rx {
            if let Ok(pings) = rx.try_recv() {
                for (server, ping) in self.servers.iter_mut().zip(pings) {
                    server.ping_ms = ping;
                }
                self.ping_rx = None;
                let responded = self.servers.iter().filter(|s| s.ping_ms.is_some()).count();
                self.status_message = format!(
                    "✓ Załadowano {} serwerów — {} odpowiedziało na ping.",
                    self.servers.len(),
                    responded
                );
            }
        }

        if let Some(result) = received {
            self.servers_loading = false;
            self.server_rx = None;
            match result {
                Ok(servers) => {
                    let count = servers.len();
                    self.servers = servers;
                    self.status_message =
                        format!("✓ Załadowano {} serwerów. Pomiar pingu...", count);
                    // Kick off background ping for all servers
                    let (ptx, prx) = std::sync::mpsc::channel();
                    self.ping_rx = Some(prx);
                    let mut servers_for_ping = self.servers.clone();
                    self.runtime.spawn(async move {
                        crate::server::ping_servers(&mut servers_for_ping).await;
                        let pings: Vec<Option<u32>> =
                            servers_for_ping.iter().map(|s| s.ping_ms).collect();
                        let _ = ptx.send(pings);
                    });
                }
                Err(e) => {
                    self.status_message = format!("✗ Błąd pobierania serwerów: {}", e);
                    log::error!("Błąd pobierania serwerów: {}", e);
                }
            }
        }
    }

    fn launch_game(&mut self) {
        let Some(idx) = self.selected_server else {
            self.status_message = "⚠ Wybierz serwer przed uruchomieniem!".into();
            return;
        };

        let server = self.servers[idx].clone();
        self.profile.last_server = Some(server.ip.clone());
        self.profile.total_launches += 1;

        if let Err(e) = self.profile.save() {
            log::warn!("Nie można zapisać profilu: {e}");
        }

        let result = if launcher::is_steam_available() {
            launcher::launch_via_steam(&server, &self.profile)
        } else {
            launcher::launch_direct(&server, &self.profile)
        };

        match result {
            Ok(_) => {
                self.status_message = format!("✓ Uruchamianie: {}", server.name);
                if self.profile.launcher_settings.close_on_launch {
                    std::process::exit(0);
                }
            }
            Err(e) => {
                self.status_message = format!("✗ Błąd: {e}");
                log::error!("{e}");
            }
        }
    }
}

impl eframe::App for DayZLauncher {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_server_results();

        ctx.set_visuals(dark_visuals());

        // Fire search fetch after 500ms debounce delay
        if let Some(since) = self.search_debounce {
            if since.elapsed() >= std::time::Duration::from_millis(500) {
                self.fetch_servers();
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }

        if self.servers_loading || self.ping_rx.is_some() {
            ctx.request_repaint();
        }
        if let UpdateStatus::Downloading { .. } | UpdateStatus::Checking = self.updater.get_status()
        {
            ctx.request_repaint();
        }

        egui::TopBottomPanel::top("title_bar")
            .exact_height(56.0)
            .show(ctx, |ui| {
                self.render_title_bar(ui);
            });

        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(28.0)
            .show(ctx, |ui| {
                self.render_status_bar(ui);
            });

        egui::SidePanel::right("launch_panel")
            .exact_width(260.0)
            .resizable(false)
            .show(ctx, |ui| {
                self.render_launch_panel(ui);
            });

        egui::CentralPanel::default().show(ctx, |ui| match self.active_tab {
            Tab::Servers => self.render_servers_tab(ui),
            Tab::Profile => self.render_profile_tab(ui),
            Tab::Settings => self.render_settings_tab(ui),
            Tab::Updates => self.render_updates_tab(ui),
        });
    }
}

impl DayZLauncher {
    fn render_title_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(16.0);

            ui.label(
                RichText::new("DAYZ")
                    .font(FontId::proportional(32.0))
                    .color(Color32::WHITE)
                    .strong(),
            );
            ui.label(
                RichText::new("LAUNCHER")
                    .font(FontId::proportional(11.0))
                    .color(RUST_COLOR)
                    .strong(),
            );

            ui.add_space(32.0);

            for (tab, label) in [
                (Tab::Servers, "⊞  SERWERY"),
                (Tab::Updates, "↻  AKTUALIZACJE"),
                (Tab::Profile, "◉  PROFIL"),
                (Tab::Settings, "⚙  USTAWIENIA"),
            ] {
                let is_active = self.active_tab == tab;
                let text = RichText::new(label)
                    .font(FontId::proportional(12.0))
                    .color(if is_active { Color32::WHITE } else { DIM_COLOR });

                let btn = egui::Button::new(text)
                    .fill(if is_active {
                        RUST_DIM
                    } else {
                        Color32::TRANSPARENT
                    })
                    .stroke(Stroke::NONE);

                if ui.add(btn).clicked() {
                    self.active_tab = tab;
                }
                ui.add_space(4.0);
            }
        });
    }

    fn render_status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(
                RichText::new(&self.status_message)
                    .font(FontId::monospace(11.0))
                    .color(DIM_COLOR),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(12.0);
                ui.label(
                    RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                        .font(FontId::monospace(10.0))
                        .color(VERY_DIM),
                );
            });
        });
    }

    fn render_launch_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(16.0);

        ui.vertical_centered(|ui| {
            section_header(ui, "WYBRANY SERWER");
            ui.add_space(8.0);

            if let Some(idx) = self.selected_server {
                let s = &self.servers[idx];
                ui.label(
                    RichText::new(&s.name)
                        .font(FontId::proportional(13.0))
                        .color(Color32::WHITE)
                        .strong(),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("Kraj: {}", s.country))
                        .font(FontId::proportional(11.0))
                        .color(DIM_COLOR),
                );
                ui.label(
                    RichText::new(&s.ip)
                        .font(FontId::monospace(10.0))
                        .color(VERY_DIM),
                );
                ui.label(
                    RichText::new(format!("Graczy: {}/{}", s.players, s.max_players))
                        .font(FontId::proportional(11.0))
                        .color(DIM_COLOR),
                );
                if let Some(ping) = s.ping_ms {
                    ui.label(
                        RichText::new(format!("Ping: {} ms", ping))
                            .font(FontId::proportional(11.0))
                            .color(s.ping_color()),
                    );
                }

                ui.add_space(8.0);

                let ratio = s.fill_ratio();
                let bar_color = if ratio > 0.9 {
                    Color32::from_rgb(220, 60, 40)
                } else if ratio > 0.6 {
                    Color32::from_rgb(220, 180, 50)
                } else {
                    Color32::from_rgb(80, 160, 80)
                };

                let desired = Vec2::new(220.0, 6.0);
                let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, Rounding::same(3.0), PANEL_BG);
                let fill_rect = egui::Rect::from_min_size(
                    rect.min,
                    Vec2::new(rect.width() * ratio, rect.height()),
                );
                ui.painter()
                    .rect_filled(fill_rect, Rounding::same(3.0), bar_color);
            } else {
                ui.label(
                    RichText::new("Nie wybrano serwera")
                        .font(FontId::proportional(12.0))
                        .color(DIM_COLOR),
                );
            }
        });

        ui.separator();
        ui.add_space(8.0);

        section_header(ui, "PROFIL");
        ui.add_space(4.0);
        ui.label(
            RichText::new(&self.profile.display_name)
                .font(FontId::proportional(14.0))
                .color(Color32::WHITE)
                .strong(),
        );
        ui.label(
            RichText::new(format!("Uruchomień: {}", self.profile.total_launches))
                .font(FontId::proportional(11.0))
                .color(DIM_COLOR),
        );

        ui.separator();
        ui.add_space(8.0);

        section_header(ui, "STAN GRY");
        ui.add_space(4.0);

        match self.updater.get_status() {
            UpdateStatus::Idle => {
                ui.label(
                    RichText::new("Nie sprawdzono")
                        .color(DIM_COLOR)
                        .font(FontId::proportional(11.0)),
                );
            }
            UpdateStatus::Checking => {
                ui.label(
                    RichText::new("⟳ Sprawdzanie...")
                        .color(RUST_COLOR)
                        .font(FontId::proportional(11.0)),
                );
            }
            UpdateStatus::UpToDate { current_version } => {
                ui.label(
                    RichText::new(format!("✓ Aktualna: {}", current_version))
                        .color(Color32::from_rgb(80, 200, 90))
                        .font(FontId::proportional(11.0)),
                );
            }
            UpdateStatus::UpdateAvailable { new_version, .. } => {
                ui.label(
                    RichText::new(format!("⚡ Dostępna: {}", new_version))
                        .color(Color32::from_rgb(220, 180, 50))
                        .font(FontId::proportional(11.0)),
                );
            }
            UpdateStatus::Downloading {
                progress,
                downloaded_mb,
                total_mb,
                ..
            } => {
                ui.label(
                    RichText::new(format!("▼ {:.0}/{:.0} MB", downloaded_mb, total_mb))
                        .color(RUST_COLOR)
                        .font(FontId::proportional(11.0)),
                );
                ui.add(
                    egui::ProgressBar::new(progress)
                        .desired_width(220.0)
                        .desired_height(6.0)
                        .fill(RUST_COLOR),
                );
            }
            UpdateStatus::ReadyToInstall { .. } => {
                ui.label(
                    RichText::new("✓ Gotowa do instalacji")
                        .color(Color32::from_rgb(80, 200, 90))
                        .font(FontId::proportional(11.0)),
                );
            }
            UpdateStatus::Error(e) => {
                ui.label(
                    RichText::new(format!("✗ {}", e))
                        .color(Color32::from_rgb(220, 60, 40))
                        .font(FontId::proportional(10.0)),
                );
            }
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.add_space(16.0);

            let can_launch = self.selected_server.is_some();

            let btn = egui::Button::new(
                RichText::new("▶  URUCHOM GRĘ")
                    .font(FontId::proportional(16.0))
                    .color(if can_launch {
                        Color32::WHITE
                    } else {
                        DIM_COLOR
                    })
                    .strong(),
            )
            .fill(if can_launch { RUST_COLOR } else { PANEL_BG })
            .min_size(Vec2::new(230.0, 52.0));

            if ui.add_enabled(can_launch, btn).clicked() {
                self.launch_game();
            }

            ui.add_space(4.0);

            if !can_launch {
                ui.label(
                    RichText::new("← Wybierz serwer z listy")
                        .font(FontId::proportional(10.0))
                        .color(VERY_DIM),
                );
            }
        });
    }

    fn render_servers_tab(&mut self, ui: &mut egui::Ui) {
        egui::TopBottomPanel::top("filters_panel")
            .exact_height(44.0)
            .frame(
                egui::Frame::none()
                    .fill(PANEL_BG)
                    .inner_margin(egui::Margin::symmetric(12.0, 8.0)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    let search = egui::TextEdit::singleline(&mut self.filters.search_query)
                        .hint_text("🔍 Szukaj po nazwie, mapie, IP...")
                        .desired_width(280.0)
                        .font(FontId::proportional(12.0));
                    let search_resp = ui.add(search);
                    // Arm debounce on any change; clear immediately for empty query
                    if search_resp.changed() {
                        if self.filters.search_query.is_empty() {
                            self.fetch_servers();
                        } else {
                            self.search_debounce = Some(std::time::Instant::now());
                        }
                    }

                    ui.separator();

                    ui.checkbox(
                        &mut self.filters.hide_full,
                        RichText::new("Ukryj pełne").font(FontId::proportional(11.0)),
                    );
                    ui.checkbox(
                        &mut self.filters.only_compatible_mods,
                        RichText::new("Tylko kompatybilne mody").font(FontId::proportional(11.0)),
                    );

                    ui.separator();

                    ui.label(
                        RichText::new("Max ping:")
                            .font(FontId::proportional(11.0))
                            .color(DIM_COLOR),
                    );
                    for (label, val) in [("Brak", None), ("<60ms", Some(60)), ("<120ms", Some(120))]
                    {
                        let selected = self.filters.max_ping == val;
                        let text = RichText::new(label)
                            .font(FontId::proportional(11.0))
                            .color(if selected { Color32::WHITE } else { DIM_COLOR });
                        if ui
                            .add(egui::Button::new(text).fill(if selected {
                                RUST_DIM
                            } else {
                                Color32::TRANSPARENT
                            }))
                            .clicked()
                        {
                            self.filters.max_ping = val;
                        }
                    }

                    ui.separator();

                    let refresh_btn = egui::Button::new(
                        RichText::new(if self.servers_loading {
                            "⟳ Ładowanie..."
                        } else {
                            "⟳ Odśwież"
                        })
                        .font(FontId::proportional(11.0)),
                    )
                    .fill(RUST_DIM);

                    if ui.add_enabled(!self.servers_loading, refresh_btn).clicked() {
                        self.fetch_servers();
                    }
                });
            });

        if self.servers_loading {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.label(
                    RichText::new("⟳ Pobieranie listy serwerów z BattleMetrics...")
                        .font(FontId::proportional(16.0))
                        .color(RUST_COLOR),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new("(może potrwać kilka sekund)")
                        .font(FontId::proportional(11.0))
                        .color(DIM_COLOR),
                );
            });
            return;
        }

        // Column headers
        egui::Frame::none()
            .fill(Color32::from_rgb(18, 18, 16))
            .inner_margin(egui::Margin::symmetric(12.0, 4.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    col_header(ui, "NAZWA SERWERA", 280.0);
                    col_header(ui, "IP:PORT", 140.0);
                    col_header(ui, "KRAJ", 100.0);
                    col_header(ui, "GRACZE", 70.0);
                    col_header(ui, "PING", 60.0);
                    col_header(ui, "TYP", 85.0);
                    col_header(ui, "MODY", 55.0);
                });
            });

        let filters = self.filters.clone();
        let filtered: Vec<usize> = self
            .servers
            .iter()
            .enumerate()
            .filter(|(_, s)| s.matches_filters(&filters))
            .map(|(i, _)| i)
            .collect();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let filtered_clone = filtered.clone();
                for (row, &idx) in filtered_clone.iter().enumerate() {
                    let is_selected = self.selected_server == Some(idx);
                    let server = &self.servers[idx];

                    let row_color = if is_selected {
                        RUST_DIM
                    } else if row % 2 == 0 {
                        Color32::from_rgb(20, 20, 18)
                    } else {
                        Color32::from_rgb(24, 24, 22)
                    };

                    // Allocate the full row rect with click sense — no child widgets,
                    // everything is painted directly so text is never selectable
                    let desired_height = 32.0;
                    let available_width = ui.available_width();
                    let (row_rect, row_resp) = ui.allocate_exact_size(
                        egui::vec2(available_width, desired_height),
                        egui::Sense::click(),
                    );

                    // Background + hover highlight
                    let bg = if is_selected {
                        row_color
                    } else if row_resp.hovered() {
                        Color32::from_rgb(35, 32, 28)
                    } else {
                        row_color
                    };
                    ui.painter().rect_filled(row_rect, egui::Rounding::ZERO, bg);

                    // Helper: paint one text cell clipped to its column width
                    let paint_cell =
                        |x_offset: f32, width: f32, text: &str, font: FontId, color: Color32| {
                            let cell_rect = egui::Rect::from_min_size(
                                egui::pos2(row_rect.min.x + x_offset, row_rect.min.y),
                                egui::vec2(width - 4.0, desired_height), // 4px padding between cols
                            );
                            // layout_no_wrap then clip via a clipped painter — text never bleeds
                            let galley =
                                ui.fonts(|f| f.layout_no_wrap(text.to_string(), font, color));
                            let text_pos = egui::pos2(
                                cell_rect.min.x + 2.0,
                                cell_rect.center().y - galley.size().y / 2.0,
                            );
                            // Use a painter scoped to the cell rect so text is clipped automatically
                            ui.painter()
                                .with_clip_rect(cell_rect)
                                .galley(text_pos, galley, color);
                        };

                    let name_color = if is_selected {
                        Color32::WHITE
                    } else {
                        Color32::from_rgb(200, 195, 180)
                    };
                    let player_color = if server.fill_ratio() > 0.9 {
                        Color32::from_rgb(220, 60, 40)
                    } else {
                        DIM_COLOR
                    };
                    let ping_text = server
                        .ping_ms
                        .map(|p| format!("{} ms", p))
                        .unwrap_or_else(|| "—".into());
                    let type_color = match server.server_type {
                        ServerType::Official => Color32::from_rgb(80, 140, 200),
                        ServerType::Community => Color32::from_rgb(130, 200, 80),
                        ServerType::Modded => Color32::from_rgb(200, 140, 60),
                    };
                    let mod_text = if server.mods.is_empty() {
                        "Brak".into()
                    } else {
                        server.mods.len().to_string()
                    };
                    let mod_color = if !server.mods.is_empty() && !server.mods_installed {
                        Color32::from_rgb(220, 60, 40)
                    } else {
                        DIM_COLOR
                    };

                    // x offsets match the col_header widths exactly
                    let x = 12.0;
                    paint_cell(
                        x,
                        280.0,
                        &server.name,
                        FontId::proportional(12.0),
                        name_color,
                    );
                    paint_cell(
                        x + 280.0,
                        140.0,
                        &server.ip,
                        FontId::monospace(10.0),
                        VERY_DIM,
                    );
                    paint_cell(
                        x + 420.0,
                        100.0,
                        &server.country,
                        FontId::proportional(11.0),
                        DIM_COLOR,
                    );
                    paint_cell(
                        x + 520.0,
                        70.0,
                        &format!("{}/{}", server.players, server.max_players),
                        FontId::proportional(11.0),
                        player_color,
                    );
                    paint_cell(
                        x + 590.0,
                        60.0,
                        &ping_text,
                        FontId::proportional(11.0),
                        server.ping_color(),
                    );
                    paint_cell(
                        x + 650.0,
                        85.0,
                        server.server_type.label(),
                        FontId::proportional(11.0),
                        type_color,
                    );
                    paint_cell(
                        x + 735.0,
                        55.0,
                        &mod_text,
                        FontId::proportional(11.0),
                        mod_color,
                    );

                    if row_resp.clicked() {
                        self.selected_server = Some(idx);
                        self.status_message = format!(
                            "Wybrano: {} | {}",
                            self.servers[idx].name, self.servers[idx].ip
                        );
                    }
                }

                if filtered.is_empty() && !self.servers.is_empty() {
                    ui.add_space(40.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("Brak serwerów spełniających kryteria.")
                                .color(DIM_COLOR)
                                .font(FontId::proportional(13.0)),
                        );
                    });
                }
            });

        ui.label(
            RichText::new(format!(
                "Wyświetlono {} z {} serwerów",
                filtered.len(),
                self.servers.len()
            ))
            .font(FontId::proportional(10.0))
            .color(VERY_DIM),
        );
    }

    fn render_profile_tab(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(16.0);

            section_card(ui, "PROFIL GRACZA", |ui| {
                form_row(ui, "Nazwa wyświetlana:", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.profile.display_name)
                            .desired_width(300.0)
                            .font(FontId::proportional(13.0)),
                    );
                });
                ui.add_space(8.0);
                form_row(ui, "Uruchomień gry:", |ui| {
                    ui.label(
                        RichText::new(self.profile.total_launches.to_string())
                            .color(Color32::WHITE)
                            .font(FontId::proportional(13.0)),
                    );
                });
                form_row(ui, "Ostatni serwer:", |ui| {
                    let last = self.profile.last_server.as_deref().unwrap_or("—");
                    ui.label(
                        RichText::new(last)
                            .color(DIM_COLOR)
                            .font(FontId::proportional(12.0)),
                    );
                });
                ui.add_space(8.0);
                form_row(ui, "Ulubione serwery:", |ui| {
                    let count = self.profile.favorite_servers.len();
                    ui.label(
                        RichText::new(format!("{} serwerów", count))
                            .color(DIM_COLOR)
                            .font(FontId::proportional(12.0)),
                    );
                });
            });

            ui.add_space(8.0);

            section_card(ui, "ŚCIEŻKA DO GRY", |ui| {
                form_row(ui, "Lokalizacja DayZ:", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.profile.game_path)
                            .desired_width(380.0)
                            .font(FontId::monospace(11.0)),
                    );
                    if ui.button("Otwórz folder").clicked() {
                        if let Err(e) = launcher::open_game_folder(&self.profile.game_path) {
                            self.status_message = format!("Błąd: {e}");
                        }
                    }
                });
                ui.add_space(4.0);
                if self.profile.is_game_path_valid() {
                    ui.label(
                        RichText::new("✓ DayZ_x64.exe znaleziony")
                            .color(Color32::from_rgb(80, 200, 90))
                            .font(FontId::proportional(11.0)),
                    );
                } else {
                    ui.label(
                        RichText::new("✗ Nie znaleziono DayZ_x64.exe — sprawdź ścieżkę")
                            .color(Color32::from_rgb(220, 80, 60))
                            .font(FontId::proportional(11.0)),
                    );
                }
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.add(primary_button("Zapisz profil")).clicked() {
                    match self.profile.save() {
                        Ok(_) => self.status_message = "✓ Profil zapisany.".into(),
                        Err(e) => self.status_message = format!("✗ Błąd: {e}"),
                    }
                }
                ui.add_space(8.0);
                if ui.button("Przywróć domyślne").clicked() {
                    self.profile = PlayerProfile::default();
                    self.status_message = "Profil przywrócony do domyślnych.".into();
                }
            });
        });
    }

    fn render_settings_tab(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(16.0);

            section_card(ui, "USTAWIENIA LAUNCHERA", |ui| {
                ui.checkbox(
                    &mut self.profile.launcher_settings.close_on_launch,
                    "Zamknij launcher po uruchomieniu gry",
                );
                ui.add_space(4.0);
                ui.checkbox(
                    &mut self.profile.launcher_settings.auto_check_updates,
                    "Automatycznie sprawdzaj aktualizacje przy starcie",
                );
                ui.add_space(4.0);
                ui.checkbox(
                    &mut self.profile.launcher_settings.minimize_to_tray,
                    "Minimalizuj do zasobnika systemowego (tray)",
                );
            });

            ui.add_space(8.0);

            section_card(ui, "USTAWIENIA GRY", |ui| {
                form_row(ui, "Język gry:", |ui| {
                    for lang in crate::profile::GameLanguage::all() {
                        let selected = &self.profile.game_settings.language == lang;
                        let btn = egui::Button::new(
                            RichText::new(lang.label()).font(FontId::proportional(11.0)),
                        )
                        .fill(if selected {
                            RUST_DIM
                        } else {
                            Color32::TRANSPARENT
                        });
                        if ui.add(btn).clicked() {
                            self.profile.game_settings.language = lang.clone();
                        }
                    }
                });
                ui.add_space(6.0);
                form_row(ui, "Liczba wątków CPU:", |ui| {
                    ui.add(
                        egui::Slider::new(&mut self.profile.game_settings.cpu_count, 0..=32)
                            .text("(0 = auto)"),
                    );
                });
                ui.add_space(4.0);
                ui.checkbox(
                    &mut self.profile.game_settings.file_patching,
                    "Włącz file patching (-filePatching)",
                );
                ui.add_space(2.0);
                ui.checkbox(
                    &mut self.profile.game_settings.show_script_errors,
                    "Pokazuj błędy skryptów (-showScriptErrors)",
                );
                ui.add_space(6.0);
                form_row(ui, "Dodatkowe parametry:", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(
                            &mut self.profile.game_settings.extra_launch_params,
                        )
                        .hint_text("-noBenchmark -skipIntro")
                        .desired_width(320.0)
                        .font(FontId::monospace(11.0)),
                    );
                });
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.add(primary_button("Zapisz ustawienia")).clicked() {
                    match self.profile.save() {
                        Ok(_) => self.status_message = "✓ Ustawienia zapisane.".into(),
                        Err(e) => self.status_message = format!("✗ {e}"),
                    }
                }
            });
        });
    }

    fn render_updates_tab(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(16.0);

            section_card(ui, "AKTUALIZACJE GRY", |ui| {
                let status = self.updater.get_status();

                match &status {
                    UpdateStatus::Idle => {
                        ui.label(
                            RichText::new("Naciśnij przycisk aby sprawdzić aktualizacje.")
                                .color(DIM_COLOR)
                                .font(FontId::proportional(12.0)),
                        );
                    }
                    UpdateStatus::Checking => {
                        ui.label(
                            RichText::new("⟳ Sprawdzanie dostępności aktualizacji...")
                                .color(RUST_COLOR)
                                .font(FontId::proportional(13.0)),
                        );
                    }
                    UpdateStatus::UpToDate { current_version } => {
                        ui.label(
                            RichText::new(format!(
                                "✓ Gra jest aktualna — wersja {}",
                                current_version
                            ))
                            .color(Color32::from_rgb(80, 200, 90))
                            .font(FontId::proportional(13.0))
                            .strong(),
                        );
                    }
                    UpdateStatus::UpdateAvailable {
                        current_version,
                        new_version,
                        size_mb,
                        changelog,
                    } => {
                        ui.label(
                            RichText::new(format!(
                                "⚡ Dostępna aktualizacja: {} → {}",
                                current_version, new_version
                            ))
                            .color(Color32::from_rgb(220, 180, 50))
                            .font(FontId::proportional(13.0))
                            .strong(),
                        );
                        ui.label(
                            RichText::new(format!("Rozmiar: {:.0} MB", size_mb))
                                .color(DIM_COLOR)
                                .font(FontId::proportional(11.0)),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Co nowego:")
                                .color(Color32::WHITE)
                                .font(FontId::proportional(12.0))
                                .strong(),
                        );
                        ui.label(
                            RichText::new(changelog.as_str())
                                .color(DIM_COLOR)
                                .font(FontId::proportional(11.0)),
                        );
                        ui.add_space(12.0);
                        if ui.add(primary_button("▼  Pobierz aktualizację")).clicked() {
                            self.updater.start_download();
                        }
                    }
                    UpdateStatus::Downloading {
                        progress,
                        speed_mb_s,
                        downloaded_mb,
                        total_mb,
                    } => {
                        ui.label(
                            RichText::new(format!(
                                "▼ Pobieranie: {:.0}/{:.0} MB  ({:.1} MB/s)",
                                downloaded_mb, total_mb, speed_mb_s
                            ))
                            .color(RUST_COLOR)
                            .font(FontId::proportional(13.0)),
                        );
                        ui.add_space(8.0);
                        ui.add(
                            egui::ProgressBar::new(*progress)
                                .desired_width(500.0)
                                .desired_height(12.0)
                                .text(format!("{:.0}%", progress * 100.0))
                                .fill(RUST_COLOR),
                        );
                    }
                    UpdateStatus::ReadyToInstall { version } => {
                        ui.label(
                            RichText::new(format!(
                                "✓ Pobrano wersję {} — gotowa do instalacji",
                                version
                            ))
                            .color(Color32::from_rgb(80, 200, 90))
                            .font(FontId::proportional(13.0)),
                        );
                        ui.add_space(8.0);
                        if ui.add(primary_button("⚙  Zainstaluj teraz")).clicked() {
                            self.updater.install_update();
                        }
                    }
                    UpdateStatus::Error(msg) => {
                        ui.label(
                            RichText::new(format!("✗ Błąd: {}", msg))
                                .color(Color32::from_rgb(220, 60, 40))
                                .font(FontId::proportional(12.0)),
                        );
                    }
                }

                ui.add_space(12.0);

                if !matches!(
                    status,
                    UpdateStatus::Downloading { .. } | UpdateStatus::Checking
                ) {
                    if ui
                        .button(
                            RichText::new("⟳  Sprawdź ponownie").font(FontId::proportional(12.0)),
                        )
                        .clicked()
                    {
                        self.updater.check_for_updates();
                    }
                }
            });
        });
    }
}

// ----------------------------------------------------------
// Kolory
// ----------------------------------------------------------
const RUST_COLOR: Color32 = Color32::from_rgb(192, 58, 0);
const RUST_DIM: Color32 = Color32::from_rgb(80, 25, 5);
const PANEL_BG: Color32 = Color32::from_rgb(22, 22, 20);
const DIM_COLOR: Color32 = Color32::from_rgb(130, 125, 110);
const VERY_DIM: Color32 = Color32::from_rgb(70, 68, 60);

fn section_header(ui: &mut egui::Ui, title: &str) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(
            RichText::new(title)
                .font(FontId::proportional(10.0))
                .color(RUST_COLOR)
                .strong(),
        );
    });
}

fn section_card(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::none()
        .fill(PANEL_BG)
        .inner_margin(egui::Margin::same(16.0))
        .stroke(Stroke::new(1.0, Color32::from_rgb(60, 55, 45)))
        .rounding(Rounding::same(4.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(title)
                    .font(FontId::proportional(10.0))
                    .color(RUST_COLOR)
                    .strong(),
            );
            ui.add(egui::Separator::default().spacing(8.0));
            content(ui);
        });
    ui.add_space(4.0);
}

fn form_row(ui: &mut egui::Ui, label: &str, content: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [180.0, 20.0],
            egui::Label::new(
                RichText::new(label)
                    .color(DIM_COLOR)
                    .font(FontId::proportional(12.0)),
            ),
        );
        content(ui);
    });
    ui.add_space(2.0);
}

fn col_header(ui: &mut egui::Ui, text: &str, width: f32) {
    ui.add_sized(
        [width, 16.0],
        egui::Label::new(
            RichText::new(text)
                .font(FontId::proportional(9.0))
                .color(VERY_DIM)
                .strong(),
        ),
    );
}

fn primary_button(label: &str) -> egui::Button<'static> {
    egui::Button::new(
        RichText::new(label.to_string())
            .font(FontId::proportional(12.0))
            .color(Color32::WHITE),
    )
    .fill(RUST_COLOR)
    .min_size(Vec2::new(0.0, 32.0))
}

fn dark_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = Color32::from_rgb(16, 16, 14);
    v.window_fill = Color32::from_rgb(20, 20, 18);
    v.faint_bg_color = Color32::from_rgb(22, 22, 20);
    v.extreme_bg_color = Color32::from_rgb(12, 12, 10);
    v.selection.bg_fill = Color32::from_rgb(80, 25, 5);
    v.selection.stroke = Stroke::new(1.0, RUST_COLOR);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(45, 43, 38));
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(60, 55, 45));
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, RUST_COLOR);
    v.widgets.active.bg_stroke = Stroke::new(1.0, Color32::from_rgb(220, 80, 20));
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(30, 28, 24);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(40, 35, 28);
    v.widgets.active.weak_bg_fill = RUST_DIM;
    v
}
