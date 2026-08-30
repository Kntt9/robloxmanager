//! Statistics dashboard — real-time account status based on actual running
//! Roblox instances, not cached server-side presence data. The dashboard
//! receives the tracked process list directly from the instance registry so
//! "online" means "has a Roblox client running on this machine right now".

use std::collections::{HashMap, HashSet};

use eframe::egui;
use ram_core::instances::TrackedInstance;
use ram_core::models::AccountStore;

use crate::i18n::{self, LangUi};
use crate::theme::ThemeUi;

/// Which accounts are actually running Roblox right now, derived from the
/// tracked-instance registry.
fn active_user_ids(instances: &[TrackedInstance]) -> HashSet<u64> {
    instances.iter().map(|i| i.user_id).collect()
}

/// Presence buckets based on real running instances, not on the server-side
/// presence API (which can report "online" even when no local client exists).
fn real_presence(
    store: &AccountStore,
    instances: &[TrackedInstance],
) -> (usize, usize, usize, usize, usize, usize) {
    let active = active_user_ids(instances);
    let total = store.accounts.len();
    let mut online = 0usize;
    let mut in_game = 0usize;
    let mut in_studio = 0usize;

    for acc in &store.accounts {
        if !active.contains(&acc.user_id) {
            continue;
        }
        // Account has a running Roblox client → at least "online".
        match acc.last_presence.user_presence_type {
            2 => in_game += 1,
            3 => in_studio += 1,
            _ => online += 1,
        }
    }
    let offline = total.saturating_sub(active.len());
    let moderated = store
        .accounts
        .iter()
        .filter(|a| a.moderation.as_ref().is_some_and(|m| m.is_active()))
        .count();
    let expired = store.accounts.iter().filter(|a| a.cookie_expired).count();
    (online, in_game, in_studio, offline, moderated, expired)
}

/// Accounts per group, so the panel can show how the roster is organised.
fn groups_of(store: &AccountStore) -> HashMap<&str, usize> {
    let mut groups: HashMap<&str, usize> = HashMap::new();
    for account in &store.accounts {
        if account.group.is_empty() {
            continue;
        }
        *groups.entry(&account.group).or_insert(0) += 1;
    }
    groups
}

/// Draw the statistics dashboard. Returns no actions — it is a view only.
pub fn show(
    ui: &mut egui::Ui,
    store: &AccountStore,
    running_instances: usize,
    instances: &[TrackedInstance],
) {
    let theme = ui.theme();
    let lang = ui.lang();
    let t = |key: &'static str| -> &'static str { i18n::tr(lang, key) };

    let (online, in_game, in_studio, offline, moderated, expired) =
        real_presence(store, instances);
    let total = store.accounts.len();

    egui::ScrollArea::vertical().show(ui, |ui| {
        // ---- Header ----
        ui.horizontal(|ui| {
            ui.heading(t("Statistics"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.colored_label(
                    if running_instances > 0 {
                        theme.info
                    } else {
                        ui.visuals().weak_text_color()
                    },
                    format!("{} Roblox", running_instances),
                );
            });
        });
        ui.separator();
        ui.add_space(8.0);

        if total == 0 {
            ui.label(
                egui::RichText::new(t("Add some accounts to see statistics here."))
                    .color(ui.visuals().weak_text_color()),
            );
            return;
        }

        // ---- Metric cards in a row ----
        let card_frame = egui::Frame::default()
            .inner_margin(egui::Margin::same(10.0))
            .rounding(egui::Rounding::same(8.0))
            .fill(ui.visuals().extreme_bg_color);

        ui.horizontal_wrapped(|ui| {
            metric_card(ui, &card_frame, &theme, t("Total"), total, egui::Color32::from_gray(200), t("Every account in RM"));
            metric_card(ui, &card_frame, &theme, t("Online"), online, egui::Color32::from_rgb(60, 180, 75), t("Accounts with a running Roblox client"));
            metric_card(ui, &card_frame, &theme, t("In Game"), in_game, egui::Color32::from_rgb(30, 144, 255), t("Accounts currently in a Roblox game"));
            metric_card(ui, &card_frame, &theme, t("In Studio"), in_studio, egui::Color32::from_rgb(255, 165, 0), t("Accounts open in Roblox Studio"));
            metric_card(ui, &card_frame, &theme, t("Offline"), offline, egui::Color32::from_gray(130), t("Accounts with no Roblox client running"));
            metric_card(ui, &card_frame, &theme, t("Moderated"), moderated, egui::Color32::from_rgb(230, 130, 40), t("Accounts with an active moderation/termination"));
            metric_card(ui, &card_frame, &theme, t("Cookie Expired"), expired, egui::Color32::from_rgb(200, 60, 60), t("Accounts whose cookie no longer validates"));
        });

        ui.add_space(10.0);

        // ---- Presence bar ----
        card_frame.show(ui, |ui: &mut egui::Ui| {
            ui.set_min_width(ui.available_width());
            ui.strong(t("Presence"));
            ui.add_space(6.0);
            draw_presence_bar(ui, store, instances, lang, theme);
            ui.add_space(6.0);
            // Legend
            ui.horizontal_wrapped(|ui| {
                if online > 0 { legend_dot(ui, t("Online"), online, egui::Color32::from_rgb(60, 180, 75), lang); }
                if in_game > 0 { legend_dot(ui, t("In Game"), in_game, egui::Color32::from_rgb(30, 144, 255), lang); }
                if in_studio > 0 { legend_dot(ui, t("In Studio"), in_studio, egui::Color32::from_rgb(255, 165, 0), lang); }
                if offline > 0 { legend_dot(ui, t("Offline"), offline, egui::Color32::from_gray(130), lang); }
            });
        });
        ui.add_space(6.0);

        // ---- Groups breakdown ----
        let groups = groups_of(store);
        if !groups.is_empty() {
            card_frame.show(ui, |ui: &mut egui::Ui| {
                ui.set_min_width(ui.available_width());
                ui.strong(t("Accounts by Group"));
                ui.add_space(6.0);
                egui::Grid::new("stats_groups")
                    .num_columns(2)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        for (name, count) in groups.iter() {
                            ui.label(format!("{name}"));
                            ui.label(format!("{count}"));
                            ui.end_row();
                        }
                    });
            });
            ui.add_space(6.0);
        }
    });
}

/// A single metric card showing a big number and a label.
fn metric_card(
    ui: &mut egui::Ui,
    frame: &egui::Frame,
    _theme: &crate::theme::Theme,
    label: &str,
    count: usize,
    color: egui::Color32,
    tooltip: &str,
) {
    frame.show(ui, |ui| {
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(format!("{}", count))
                    .heading()
                    .strong()
                    .size(24.0)
                    .color(color),
            );
            ui.label(
                egui::RichText::new(label)
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
        })
        .response
        .on_hover_text(tooltip);
    });
}

/// A horizontal stacked bar showing the online/offline split proportionally.
fn draw_presence_bar(
    ui: &mut egui::Ui,
    store: &AccountStore,
    instances: &[TrackedInstance],
    lang: crate::i18n::Language,
    theme: crate::theme::Theme,
) {
    let (online, in_game, in_studio, offline, _, _) = real_presence(store, instances);
    let total = store.accounts.len().max(1) as f32;
    let buckets = [
        (in_game as f32 / total, egui::Color32::from_rgb(30, 144, 255)),
        (online as f32 / total, egui::Color32::from_rgb(60, 180, 75)),
        (in_studio as f32 / total, egui::Color32::from_rgb(255, 165, 0)),
        (offline as f32 / total, egui::Color32::from_gray(130)),
    ];

    let avail = ui.available_width();
    let height = 22.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(avail, height), egui::Sense::hover());
    let painter = ui.painter();

    // Background track
    painter.rect_filled(rect, egui::Rounding::same(4.0), theme.surface);

    // Stacked segments
    let mut x = rect.left();
    for (frac, color) in &buckets {
        if *frac <= 0.0 {
            continue;
        }
        let w = rect.width() * frac;
        let seg = egui::Rect::from_min_max(
            egui::pos2(x, rect.top()),
            egui::pos2(x + w, rect.bottom()),
        );
        painter.rect_filled(seg, egui::Rounding::same(2.0), *color);
        x += w;
    }

    // Center label
    let active = instances.len();
    let text = format!("{}/{} {}", active, store.accounts.len(), i18n::tr(lang, "online"));
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(12.0),
        egui::Color32::WHITE,
    );
    ui.response()
        .on_hover_text(i18n::tr(lang, "Split of accounts by current status"));
}

/// A small colored dot + label, for the legend.
fn legend_dot(ui: &mut egui::Ui, label: &str, count: usize, color: egui::Color32, _lang: crate::i18n::Language) {
    ui.horizontal(|ui| {
        let (dot_rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
        ui.painter().circle_filled(dot_rect.center(), 4.0, color);
        ui.label(
            egui::RichText::new(format!("{} ({})", label, count))
                .small()
                .color(ui.visuals().weak_text_color()),
        );
    });
}