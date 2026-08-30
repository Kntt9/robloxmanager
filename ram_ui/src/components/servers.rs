//! Servers panel — public servers for the Place ID currently on the account
//! card, with ordering, a "hide full" filter, pagination, and highlighted
//! servers where other RM accounts are currently playing.

use eframe::egui;
use ram_core::api::GameServer;
use ram_core::models::AccountStore;

use crate::i18n::{self, LangUi};
use crate::theme::ThemeUi;

/// Sort orders for the server list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerSort {
    Ping,
    Empty,
    FewestPlayers,
    MostFreeSlots,
}

/// Actions the servers panel can request from the app.
pub enum ServersAction {
    /// (Re)fetch the first page for `place_id`.
    Refresh(u64),
    /// Fetch the next page for `place_id`.
    LoadMore(u64),
    /// Launch into `place_id` / `job_id`.
    Join { place_id: u64, job_id: String },
    /// Close the panel.
    Close,
}

/// Persistent UI state for the panel (sort, filter).
#[derive(Default)]
pub struct ServersState {
    pub sort: ServerSort,
    pub hide_full: bool,
}

impl Default for ServerSort {
    fn default() -> Self {
        ServerSort::Ping
    }
}

/// Data backing the panel: the servers accumulated so far, plus pagination.
#[derive(Default)]
pub struct ServersData {
    pub place_id: Option<u64>,
    pub servers: Vec<GameServer>,
    pub next_cursor: Option<String>,
    pub loading: bool,
    pub error: Option<String>,
    /// When the current data was last fetched successfully (wall clock).
    /// Used to avoid re-fetching within the TTL — the Roblox games API rate-
    /// limits aggressively after ~3 rapid requests.
    pub fetched_at: Option<std::time::Instant>,
    /// When the user may attempt a new request again after a rate limit.
    /// Set after a `RateLimited` error so "Try again" waits out the cooldown
    /// instead of hammering the API the instant it returns 429.
    pub retry_after: Option<std::time::Instant>,
}

/// A RM account that is currently in the game: label plus its job ID.
pub struct HighlightedAccount {
    pub label: String,
    pub game_id: String,
}

/// Draw the servers panel.
pub fn show(
    ui: &mut egui::Ui,
    state: &mut ServersState,
    data: &ServersData,
    store: &AccountStore,
    anonymize: bool,
    launch_targets: &std::collections::HashMap<u64, (u64, Option<String>, std::time::Instant)>,
) -> Option<ServersAction> {
    let theme = ui.theme();
    let lang = ui.lang();
    let t = |key: &'static str| -> &'static str { i18n::tr(lang, key) };
    let mut action: Option<ServersAction> = None;

    let place_id = data.place_id;

    // ---- Header ----
    ui.horizontal(|ui| {
        ui.heading(format!("\u{1f5a5}  {}", t("Servers")));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(format!("\u{1f503}  {}", t("Refresh")))
                .on_hover_text(t("Re-fetch servers"))
                .clicked()
            {
                if let Some(pid) = place_id {
                    action = Some(ServersAction::Refresh(pid));
                }
            }
        });
    });
    ui.label(
        egui::RichText::new(match place_id {
            Some(pid) => format!("{} {}", t("Game"), pid),
            None => t("No game selected").to_string(),
        })
        .small()
        .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(4.0);

    // ---- No Place ID ----
    let Some(pid) = place_id else {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new("\u{1f5a5}").size(28.0).color(ui.visuals().weak_text_color()));
            ui.add_space(6.0);
            ui.label(t("No game selected"));
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new(t("Enter a Place ID to view servers."))
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
        });
        return None;
    };

    // ---- Controls: sort + hide full ----
    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new(t("Sort by"))
                .small()
                .color(ui.visuals().weak_text_color()),
        );
        egui::ComboBox::from_id_salt("server_sort")
            .selected_text(sort_label(state.sort, lang))
            .width(150.0)
            .show_ui(ui, |ui| {
                for s in [ServerSort::Ping, ServerSort::Empty, ServerSort::FewestPlayers, ServerSort::MostFreeSlots] {
                    ui.selectable_value(&mut state.sort, s, sort_label(s, lang));
                }
            });
        ui.add_space(8.0);
        ui.checkbox(&mut state.hide_full, t("Hide full servers"));
    });
    ui.separator();
    ui.add_space(4.0);

    // ---- Highlighted servers (RM accounts in this game) ----
    let highlighted = highlighted_accounts(store, pid, anonymize, launch_targets);
    if !highlighted.is_empty() {
        ui.label(
            egui::RichText::new(format!("\u{2b50}  {}", t("Featured servers")))
                .strong()
                .size(15.0)
                .color(theme.warning_text),
        );
        ui.add_space(4.0);
        for acc in &highlighted {
            let card = egui::Frame::default()
                .inner_margin(egui::Margin::same(8.0))
                .rounding(egui::Rounding::same(6.0))
                .fill(theme.warning_surface)
                .stroke(egui::Stroke::new(1.0, theme.warning_text.gamma_multiply(0.5)));
            card.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(format!("\u{1f7e2}  {} {}", acc.label, t("is here")))
                                .strong()
                                .size(13.5),
                        );
                        ui.label(
                            egui::RichText::new(short_server_id(&acc.game_id))
                                .small()
                                .color(ui.visuals().weak_text_color()),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(format!("\u{25b6}  {}", t("Join")))
                            .clicked()
                        {
                            action = Some(ServersAction::Join {
                                place_id: pid,
                                job_id: acc.game_id.clone(),
                            });
                        }
                    });
                });
            });
            ui.add_space(4.0);
        }
        ui.add_space(6.0);
    }

    // ---- Loading ----
    if data.loading && data.servers.is_empty() {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.spinner();
            ui.label(t("Loading servers\u{2026}"));
        });
        return None;
    }

    // ---- Error ----
    if let Some(err) = &data.error {
        egui::Frame::default()
            .fill(theme.danger_surface)
            .rounding(egui::Rounding::same(6.0))
            .inner_margin(egui::Margin::same(10.0))
            .show(ui, |ui| {
                ui.colored_label(theme.danger_text, format!("\u{26a0} {err}"));
                ui.add_space(4.0);
                if ui.button(t("Try again")).clicked() {
                    action = Some(ServersAction::Refresh(pid));
                }
            });
        return None;
    }

    // ---- Empty ----
    if data.servers.is_empty() {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.label(t("No servers available at the moment."));
        });
        return None;
    }

    // ---- Server list ----
    let highlighted_ids: std::collections::HashSet<&str> =
        highlighted.iter().map(|h| h.game_id.as_str()).collect();

    // Order + filter.
    let mut visible: Vec<&GameServer> = data
        .servers
        .iter()
        .filter(|s| !(state.hide_full && s.is_full()))
        .filter(|s| !highlighted_ids.contains(s.id.as_str())) // never duplicate
        .collect();
    sort_servers(&mut visible, state.sort);

    if visible.is_empty() {
        ui.label(
            egui::RichText::new(t("No servers available at the moment."))
                .color(ui.visuals().weak_text_color()),
        );
    } else {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for server in visible {
                    let card = egui::Frame::default()
                        .inner_margin(egui::Margin::same(8.0))
                        .rounding(egui::Rounding::same(6.0))
                        .fill(ui.visuals().faint_bg_color)
                        .stroke(egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color));
                    card.show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("{} {}", t("Server"), short_server_id(&server.id)))
                                        .strong()
                                        .size(13.5),
                                );
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} / {} {}",
                                        server.playing,
                                        server.max_players,
                                        t("players")
                                    ))
                                    .small()
                                    .color(ui.visuals().weak_text_color()),
                                );
                                // Ping only if the API reported a real one.
                                if let Some(ping) = server.ping {
                                    ui.label(
                                        egui::RichText::new(format!("\u{25c9} {ping} ms"))
                                            .small()
                                            .color(ui.visuals().weak_text_color()),
                                    );
                                }
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let full = server.is_full();
                                ui.colored_label(
                                    if full { theme.danger_text } else { theme.success_text },
                                    if full { t("Full") } else { t("Available") },
                                );
                                ui.add_space(6.0);
                                if ui
                                    .add_enabled(
                                        !full,
                                        egui::Button::new(format!("\u{25b6}  {}", t("Join"))),
                                    )
                                    .clicked()
                                {
                                    action = Some(ServersAction::Join {
                                        place_id: pid,
                                        job_id: server.id.clone(),
                                    });
                                }
                            });
                        });
                    });
                    ui.add_space(4.0);
                }

                // Load more
                if let Some(_cursor) = &data.next_cursor {
                    ui.add_space(4.0);
                    ui.vertical_centered(|ui| {
                        if ui.button(t("Load more")).clicked() {
                            action = Some(ServersAction::LoadMore(pid));
                        }
                    });
                }
            });
    }

    action
}

/// Accounts in `store` currently in `place_id`, with their job IDs.
///
/// Two sources feed this, merged without duplicates:
///
/// 1. The Manager's own [`launch_targets`] record — what RM *sent* each account
///    to. Authoritative for accounts launched from this app: it is written the
///    moment a server launch happens and does not depend on the external
///    presence API being up to date.
/// 2. `last_presence` — Roblox's own report, used as a fallback for accounts
///    the Manager did not launch into a server itself (e.g. the user joined a
///    game by hand and the presence poll caught it).
///
/// The Manager record wins on ties (same job ID seen in both).
fn highlighted_accounts(
    store: &AccountStore,
    place_id: u64,
    anonymize: bool,
    launch_targets: &std::collections::HashMap<u64, (u64, Option<String>, std::time::Instant)>,
) -> Vec<HighlightedAccount> {
    let mut out: Vec<HighlightedAccount> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 1. Manager-launched targets: source of truth for its own launches.
    for (i, acc) in store.accounts.iter().enumerate() {
        let Some((target_place, Some(job_id), _when)) = launch_targets.get(&acc.user_id) else {
            continue;
        };
        if *target_place != place_id {
            continue;
        }
        if job_id.is_empty() {
            continue;
        }
        if !seen.insert(job_id.clone()) {
            continue;
        }
        let label = if anonymize {
            format!("Account {}", i + 1)
        } else {
            acc.label().to_string()
        };
        out.push(HighlightedAccount {
            label,
            game_id: job_id.clone(),
        });
    }

    // 2. Fallback: external presence, for accounts the Manager did not launch.
    for (i, acc) in store.accounts.iter().enumerate() {
        let p = &acc.last_presence;
        if p.user_presence_type != 2 {
            continue;
        }
        if p.place_id != Some(place_id) {
            continue;
        }
        let Some(game_id) = p.game_id.as_deref().filter(|s| !s.is_empty()) else {
            continue;
        };
        if !seen.insert(game_id.to_string()) {
            continue;
        }
        let label = if anonymize {
            format!("Account {}", i + 1)
        } else {
            acc.label().to_string()
        };
        out.push(HighlightedAccount {
            label,
            game_id: game_id.to_string(),
        });
    }
    out
}

/// Apply the current sort to `servers`.
fn sort_servers(servers: &mut Vec<&GameServer>, sort: ServerSort) {
    let key = |s: &GameServer| -> (u32, u32, u32, u32) {
        match sort {
            ServerSort::Ping => (
                s.ping.unwrap_or(u32::MAX),
                s.playing,
                s.max_players,
                free_key(s),
            ),
            ServerSort::Empty => (s.playing, s.max_players, free_key(s), s.ping.unwrap_or(0)),
            ServerSort::FewestPlayers => (s.playing, s.max_players, free_key(s), s.ping.unwrap_or(0)),
            ServerSort::MostFreeSlots => (u32::MAX - free_key(s), s.playing, s.max_players, s.ping.unwrap_or(0)),
        }
    };
    servers.sort_by_key(|s| key(s));
}

fn free_key(s: &GameServer) -> u32 {
    s.max_players.saturating_sub(s.playing)
}

fn sort_label(sort: ServerSort, lang: crate::i18n::Language) -> String {
    match sort {
        ServerSort::Ping => i18n::tr(lang, "Lowest ping"),
        ServerSort::Empty => i18n::tr(lang, "Emptiest"),
        ServerSort::FewestPlayers => i18n::tr(lang, "Fewest players"),
        ServerSort::MostFreeSlots => i18n::tr(lang, "Most free slots"),
    }
    .to_string()
}

/// Short display form of a server job ID: `#f0c04106` (first 8 chars).
fn short_server_id(id: &str) -> String {
    let trimmed: String = id.chars().filter(|c| *c != '-').take(8).collect();
    format!("#{trimmed}")
}

/// Convenience accessor for the panel's place id.
pub fn place_id_of(data: &ServersData) -> Option<u64> {
    data.place_id
}
