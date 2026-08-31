use eframe::egui;
use ram_core::api::GameServer;
use ram_core::instances::TrackedInstance;
use ram_core::models::{AccountGroup, AccountGroupStore, AccountStore};

use crate::i18n::{self, LangUi};
use crate::theme::ThemeUi;

pub const MAX_DISTRIBUTE_PAGES: u32 = 3;
pub const MAX_COLLECTED_SERVERS: usize = 60;

pub enum GroupsAction {
    OpenCreate,
    OpenEdit(String),
    CloseModal,
    SaveGroup { id: Option<String>, name: String, place_id: u64, member_user_ids: Vec<u64> },
    RequestDelete(String),
    ConfirmDelete(String),
    CancelDelete,
    JoinTogether(String),
    RequestDistribute(String),
    ConfirmDistribute,
    CancelDistribute,
    CloseDistribute,
}

#[derive(Default)]
pub struct GroupsState {
    pub modal_open: bool,
    pub editing_id: Option<String>,
    pub name_input: String,
    pub place_input: String,
    pub selected_accounts: std::collections::HashSet<u64>,
    pub search: String,
    pub confirm_delete: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistAccountState { Waiting, Found, Entering, Done, NoServer, AlreadyRunning }

#[derive(Debug, Clone)]
pub struct DistAccountStatus { pub user_id: u64, pub job_id: Option<String>, pub state: DistAccountState }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistributePhase { Confirm, Fetching, Launching, Done }

pub struct DistributeState {
    pub group_id: String, pub place_id: u64, pub seq: u64, pub phase: DistributePhase,
    pub accounts: Vec<u64>, pub statuses: Vec<DistAccountStatus>,
    pub collected: Vec<GameServer>, pub next_cursor: Option<String>, pub pages: u32,
    pub used_job_ids: std::collections::HashSet<String>, pub error: Option<String>,
    pub launched: usize, pub failed: usize,
}

// ---------------------------------------------------------------------------
// Main entry
// ---------------------------------------------------------------------------

pub fn show(
    ui: &mut egui::Ui,
    state: &mut GroupsState,
    store: &AccountGroupStore,
    accounts: &AccountStore,
    anonymize: bool,
    tracked_instances: &[TrackedInstance],
    game_preview_thumbs: &std::collections::HashMap<u64, Vec<u8>>,
    game_preview_cache: &std::collections::HashMap<u64, ram_core::api::GamePreview>,
) -> Vec<GroupsAction> {
    let theme = ui.theme();
    let lang = ui.lang();
    let t = |key: &'static str| -> &'static str { i18n::tr(lang, key) };
    let mut actions: Vec<GroupsAction> = Vec::new();

    let running_ids: std::collections::HashSet<u64> =
        tracked_instances.iter().map(|i| i.user_id).collect();

    // ---- Header ----
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(format!("\u{1f465}  {}", t("Account groups"))).heading().size(20.0));
            ui.label(egui::RichText::new(t("Organize accounts by game and run several accounts in a coordinated way.")).small().color(theme.text_muted));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let btn = egui::Button::new(egui::RichText::new(format!("\u{2795}  {}", t("Create group"))).size(14.0))
                .fill(theme.accent)
                .min_size(egui::vec2(0.0, 36.0));
            if ui.add(btn).on_hover_text(t("Create group")).clicked() {
                actions.push(GroupsAction::OpenCreate);
            }
        });
    });
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(8.0);

    // ---- Empty state ----
    if store.groups.is_empty() {
        ui.add_space(60.0);
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new("\u{1f465}").size(48.0).color(theme.text_muted));
            ui.add_space(12.0);
            ui.label(egui::RichText::new(t("No groups created")).size(16.0).strong());
            ui.add_space(4.0);
            ui.label(egui::RichText::new(t("Create a group to organize multiple accounts in the same game.")).small().color(theme.text_muted));
            ui.add_space(16.0);
            let btn = egui::Button::new(egui::RichText::new(format!("\u{2795}  {}", t("Create group"))).size(14.0))
                .fill(theme.accent).min_size(egui::vec2(0.0, 38.0));
            if ui.add(btn).clicked() { actions.push(GroupsAction::OpenCreate); }
        });
    } else {
        // ---- Grid of cards ----
        let card_gap = 12.0;
        let avail = ui.available_width();
        let cols = if avail >= 900.0 { 2 } else { 1 };
        let card_w = (avail - card_gap * (cols as f32 - 1.0)).floor();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("groups_grid")
                    .min_col_width(card_w)
                    .max_col_width(card_w)
                    .spacing([card_gap, card_gap])
                    .show(ui, |ui| {
                        for (i, group) in store.groups.iter().enumerate() {
                            if i > 0 && i % cols == 0 { ui.end_row(); }
                            actions.extend(group_card(
                                ui, group, card_w, &running_ids,
                                game_preview_thumbs, game_preview_cache,
                            ));
                        }
                    });
            });
    }

    // ---- Delete confirmation modal ----
    if let Some(id) = &state.confirm_delete {
        let mut open = true;
        egui::Window::new(t("Delete this group?"))
            .id(egui::Id::new("group_delete_confirm"))
            .collapsible(false).resizable(false).open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.label(t("This action removes the group configuration, but does not delete the accounts."));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(t("Cancel")).clicked() { actions.push(GroupsAction::CancelDelete); }
                    if ui.button(egui::RichText::new(t("Delete")).color(theme.danger_text)).clicked()
                        { actions.push(GroupsAction::ConfirmDelete(id.clone())); }
                });
            });
        if !open { actions.push(GroupsAction::CancelDelete); }
    }

    // ---- Create/edit modal ----
    if state.modal_open {
        actions.extend(group_modal(ui, state, accounts, anonymize, game_preview_thumbs, game_preview_cache));
    }

    actions
}

// ---------------------------------------------------------------------------
// Group card
// ---------------------------------------------------------------------------

fn group_card(
    ui: &mut egui::Ui,
    group: &AccountGroup,
    card_w: f32,
    running_ids: &std::collections::HashSet<u64>,
    thumbs: &std::collections::HashMap<u64, Vec<u8>>,
    cache: &std::collections::HashMap<u64, ram_core::api::GamePreview>,
) -> Vec<GroupsAction> {
    let theme = ui.theme();
    let lang = ui.lang();
    let t = |key: &'static str| -> &'static str { i18n::tr(lang, key) };
    let mut actions: Vec<GroupsAction> = Vec::new();

    let cover_bytes = thumbs.get(&group.place_id).cloned()
        .or_else(|| if group.game_thumb_bytes.is_empty() { None } else { Some(group.game_thumb_bytes.clone()) });
    let game_name = if group.game_name.is_empty() {
        cache.get(&group.place_id).map(|p| p.name.clone()).unwrap_or_default()
    } else { group.game_name.clone() };

    let gid = group.id.clone();
    let cover_size = egui::vec2(120.0, 68.0);
    let card_h = 172.0;

    let (rect, response) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::hover());
    let stroke_color = if response.hovered() {
        theme.accent_border
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke.color
    };

    let painter = ui.painter();
    painter.rect_filled(rect, egui::Rounding::same(10.0), theme.surface);
    painter.rect_stroke(rect, egui::Rounding::same(10.0), egui::Stroke::new(1.0, stroke_color));

    let inner = rect.shrink(12.0);
    let mut content = ui.child_ui(inner, egui::Layout::top_down(egui::Align::Min), None);
    content.set_clip_rect(rect.expand(60.0));

    // ---- Row 1: cover + name + menu ----
    content.horizontal(|ui| {
        let cover_rect = egui::Rect::from_min_size(ui.cursor().min, cover_size);
        ui.painter().rect_filled(cover_rect, egui::Rounding::same(8.0), theme.surface_raised);
        if let Some(bytes) = &cover_bytes {
            if !bytes.is_empty() {
                ui.put(cover_rect, egui::Image::from_bytes(format!("bytes://group_cover/{}", group.place_id), bytes.clone())
                    .fit_to_exact_size(cover_size).rounding(egui::Rounding::same(8.0)));
            } else {
                cover_placeholder(ui, cover_rect, &theme);
            }
        } else {
            cover_placeholder(ui, cover_rect, &theme);
        }
        ui.add_space(12.0);

        // Name (vertically centered next to cover)
        ui.vertical(|ui| {
            ui.set_min_width(inner.width() - cover_size.x - 12.0 - 40.0);
            ui.add_space(8.0);
            ui.label(egui::RichText::new(&group.name).strong().size(15.0));
            if !game_name.is_empty() {
                ui.label(egui::RichText::new(&game_name).size(12.5).color(theme.text_muted));
            }
        });

        // Menu (top-right)
        ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
            ui.add_space(2.0);
            egui::menu::menu_button(ui, "\u{22ee}", |ui| {
                if ui.button(t("Edit")).clicked() { actions.push(GroupsAction::OpenEdit(gid.clone())); ui.close_menu(); }
                if ui.button(egui::RichText::new(t("Delete")).color(theme.danger_text)).clicked()
                    { actions.push(GroupsAction::RequestDelete(gid.clone())); ui.close_menu(); }
            });
        });
    });

    content.add_space(10.0);

    // ---- Row 2: meta info (Place ID + accounts + running) ----
    content.horizontal(|ui| {
        ui.add_space(cover_size.x + 12.0);
        ui.label(egui::RichText::new(format!("{}: {}", t("Place ID"), group.place_id)).small().color(theme.text_muted));

        let total = group.member_user_ids.len();
        let running = group.member_user_ids.iter().filter(|uid| running_ids.contains(uid)).count();
        let accounts_text = format!("\u{1f465} {total}");
        let status_color = if running > 0 { theme.success } else { theme.text_muted };
        let status_text = format!("\u{25cf} {running}/{total} {}", t("running"));

        // Accounts + status as a pill
        let (pill_rect, _) = ui.allocate_exact_size(
            egui::vec2(accounts_text.len() as f32 * 8.0 + status_text.len() as f32 * 8.0 + 40.0, 22.0),
            egui::Sense::hover(),
        );
        let _ = pill_rect;
        ui.colored_label(theme.text_muted, accounts_text);
        ui.colored_label(status_color, status_text);
    });

    content.add_space(8.0);

    // ---- Row 3: actions ----
    content.horizontal(|ui| {
        ui.add_space(cover_size.x + 12.0);
        if ui.add(egui::Button::new(egui::RichText::new(format!("\u{25b6}  {}", t("Join together"))).size(12.5))
            .min_size(egui::vec2(0.0, 28.0)))
            .on_hover_text(t("Launch all accounts of this group into the same game.")).clicked()
        { actions.push(GroupsAction::JoinTogether(gid.clone())); }
        ui.add_space(6.0);
        if ui.add(egui::Button::new(egui::RichText::new(format!("\u{26a1}  {}", t("Distribute across servers"))).size(12.5))
            .min_size(egui::vec2(0.0, 28.0)))
            .on_hover_text(t("Puts each account in a different server.")).clicked()
        { actions.push(GroupsAction::RequestDistribute(gid.clone())); }
    });

    actions
}

fn cover_placeholder(ui: &egui::Ui, rect: egui::Rect, theme: &crate::theme::Theme) {
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "\u{1f3ae}", egui::FontId::proportional(36.0), theme.text_muted);
}

// ---------------------------------------------------------------------------
// Create / Edit modal
// ---------------------------------------------------------------------------

fn group_modal(
    ui: &mut egui::Ui,
    state: &mut GroupsState,
    accounts: &AccountStore,
    anonymize: bool,
    game_preview_thumbs: &std::collections::HashMap<u64, Vec<u8>>,
    game_preview_cache: &std::collections::HashMap<u64, ram_core::api::GamePreview>,
) -> Vec<GroupsAction> {
    let theme = ui.theme();
    let lang = ui.lang();
    let t = |key: &'static str| -> &'static str { i18n::tr(lang, key) };
    let mut actions: Vec<GroupsAction> = Vec::new();

    let title = if state.editing_id.is_some() { t("Edit group") } else { t("Create group") };
    let mut open = true;

    egui::Window::new(title)
        .id(egui::Id::new("group_modal"))
        .default_size([460.0, 520.0]).min_size([400.0, 400.0])
        .collapsible(false).open(&mut open)
        .show(ui.ctx(), |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.label(t("Group name"));
                ui.text_edit_singleline(&mut state.name_input);
                ui.add_space(6.0);
                ui.label(t("Place ID"));
                ui.text_edit_singleline(&mut state.place_input);
                ui.add_space(4.0);

                if let Ok(pid) = state.place_input.trim().parse::<u64>() {
                    let preview = game_preview_cache.get(&pid);
                    let thumb = game_preview_thumbs.get(&pid);
                    egui::Frame::default()
                        .inner_margin(egui::Margin::same(8.0)).rounding(egui::Rounding::same(8.0))
                        .fill(ui.visuals().faint_bg_color)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let sz = egui::vec2(56.0, 56.0);
                                match thumb {
                                    Some(b) if !b.is_empty() => {
                                        ui.add(egui::Image::from_bytes(format!("bytes://group_preview/{pid}"), b.clone())
                                            .fit_to_exact_size(sz).rounding(egui::Rounding::same(6.0)));
                                    }
                                    _ => { placeholder_square(ui, sz, &theme); }
                                }
                                ui.add_space(8.0);
                                ui.vertical(|ui| {
                                    if let Some(p) = preview {
                                        if p.name.is_empty() {
                                            ui.label(egui::RichText::new(t("Could not identify this game")).small().color(theme.warning_text));
                                        } else {
                                            ui.label(egui::RichText::new(&p.name).strong().size(13.5));
                                        }
                                    } else {
                                        ui.spinner();
                                        ui.label(egui::RichText::new(t("Identifying game\u{2026}")).small().color(ui.visuals().weak_text_color()));
                                    }
                                    ui.label(egui::RichText::new(format!("{}: {pid}", t("Place ID"))).small().color(ui.visuals().weak_text_color()));
                                });
                            });
                        });
                    ui.add_space(4.0);
                }

                ui.add_space(8.0);
                ui.label(egui::RichText::new(t("Select which accounts belong to this group:")).size(13.0).color(ui.visuals().weak_text_color()));
                ui.label(egui::RichText::new(format!("{} {}", state.selected_accounts.len(), t("Accounts selected"))).small().color(theme.accent_text));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(t("Search accounts")).small().color(ui.visuals().weak_text_color()));
                    ui.text_edit_singleline(&mut state.search);
                });
                ui.add_space(4.0);

                if accounts.accounts.is_empty() {
                    ui.label(egui::RichText::new(t("No accounts available")).small().color(ui.visuals().weak_text_color()));
                } else {
                    egui::Frame::default()
                        .inner_margin(egui::Margin::same(6.0)).rounding(egui::Rounding::same(6.0))
                        .fill(ui.visuals().faint_bg_color)
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical().id_salt("group_member_list").max_height(180.0)
                                .auto_shrink([false, false]).show(ui, |ui| {
                                let q = state.search.trim().to_lowercase();
                                for acc in &accounts.accounts {
                                    let label = if anonymize { format!("Account {}", acc.user_id) }
                                        else if acc.alias.is_empty() { acc.username.clone() }
                                        else { format!("{} ({})", acc.alias, acc.username) };
                                    if !q.is_empty() && !label.to_lowercase().contains(&q) && !acc.username.to_lowercase().contains(&q) { continue; }
                                    let mut checked = state.selected_accounts.contains(&acc.user_id);
                                    ui.checkbox(&mut checked, label);
                                    if checked { state.selected_accounts.insert(acc.user_id); }
                                    else { state.selected_accounts.remove(&acc.user_id); }
                                }
                            });
                        });
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button(egui::RichText::new(t("Cancel")).size(13.0)).clicked() { actions.push(GroupsAction::CloseModal); }
                    let can_save = state.place_input.trim().parse::<u64>().is_ok() && !state.name_input.trim().is_empty();
                    if ui.add_enabled(can_save, egui::Button::new(egui::RichText::new(
                        if state.editing_id.is_some() { t("Save changes") } else { t("Create") }
                    ).size(13.0))).clicked() {
                        let id = state.editing_id.clone();
                        let name = state.name_input.trim().to_string();
                        let pid = state.place_input.trim().parse::<u64>().unwrap_or(0);
                        let members: Vec<u64> = state.selected_accounts.iter().copied().collect();
                        actions.push(GroupsAction::SaveGroup { id, name, place_id: pid, member_user_ids: members });
                    }
                });
            });
        });

    if !open { actions.push(GroupsAction::CloseModal); }
    actions
}

fn placeholder_square(ui: &mut egui::Ui, size: egui::Vec2, theme: &crate::theme::Theme) {
    let (r, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_filled(r, egui::Rounding::same(6.0), theme.surface);
    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "\u{1f3ae}", egui::FontId::proportional(18.0), theme.text_muted);
}

// ---------------------------------------------------------------------------
// Distribution window
// ---------------------------------------------------------------------------

pub fn show_distribution(ctx: &egui::Context, dist: &DistributeState, accounts: &AccountStore, anonymize: bool) -> Option<GroupsAction> {
    let theme = crate::theme::Theme::default();
    let lang = i18n::of(ctx);
    let t = |key: &'static str| -> &'static str { i18n::tr(lang, key) };
    let mut action: Option<GroupsAction> = None;
    let mut open = true;

    egui::Window::new(format!("\u{26a1}  {}", t("Distributing accounts")))
        .id(egui::Id::new("group_distribute"))
        .default_size([440.0, 420.0]).min_size([380.0, 300.0])
        .collapsible(false).open(&mut open)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                match dist.phase {
                    DistributePhase::Confirm => {
                        let count = dist.accounts.len();
                        ui.label(egui::RichText::new(format!("{} {}", t("Distribute {} accounts?").replace("{}", &count.to_string()), "")).strong().size(14.0));
                        ui.add_space(4.0);
                        ui.label(format!("{}: {}", t("Place ID"), dist.place_id));
                        ui.add_space(2.0);
                        ui.label(egui::RichText::new(t("Strategy: 1 account per server")).small().color(ui.visuals().weak_text_color()));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button(t("Cancel")).clicked() { action = Some(GroupsAction::CancelDistribute); }
                            if ui.button(egui::RichText::new(t("Continue")).size(13.0)).clicked() { action = Some(GroupsAction::ConfirmDistribute); }
                        });
                    }
                    DistributePhase::Fetching => {
                        ui.label(egui::RichText::new(format!("\u{2699}  {}", t("Looking for server"))).size(13.5));
                        ui.add_space(4.0);
                        ui.horizontal(|ui| { ui.spinner(); ui.label(format!("{} {}", t("Servers found"), dist.collected.len())); });
                        ui.add_space(8.0);
                        per_account_rows(ui, dist, accounts, anonymize);
                    }
                    DistributePhase::Launching => {
                        ui.label(egui::RichText::new(format!("\u{26a1}  {}", t("Distributing accounts"))).strong().size(14.0));
                        ui.add_space(4.0);
                        let total = dist.accounts.len().max(1);
                        let done = dist.launched + dist.failed;
                        let frac = done as f32 / total as f32;
                        let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 12.0), egui::Sense::hover());
                        ui.painter().rect_filled(r, egui::Rounding::same(6.0), ui.visuals().faint_bg_color);
                        if frac > 0.0 { let w = r.width() * frac; ui.painter().rect_filled(egui::Rect::from_min_size(r.min, egui::vec2(w, r.height())), egui::Rounding::same(6.0), ui.theme().accent); }
                        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, format!("{done} / {}", dist.accounts.len()), egui::FontId::proportional(11.0), egui::Color32::WHITE);
                        ui.add_space(8.0);
                        per_account_rows(ui, dist, accounts, anonymize);
                    }
                    DistributePhase::Done => {
                        if let Some(err) = &dist.error { ui.colored_label(theme.danger_text, format!("\u{26a0} {err}")); }
                        else if dist.failed == 0 && dist.launched > 0 { ui.colored_label(theme.success_text, format!("\u{2713}  {}", t("Distribution complete"))); }
                        else { ui.colored_label(theme.warning_text, format!("\u{26a0}  {}", t("Distribution partially complete"))); }
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(format!("{}: {} / {}", t("accounts processed"), dist.launched + dist.failed, dist.accounts.len())).small());
                        if !dist.used_job_ids.is_empty() { ui.label(egui::RichText::new(format!("{}: {}", t("Different servers used"), dist.used_job_ids.len())).small().color(ui.visuals().weak_text_color())); }
                        let skipped = dist.accounts.len() - dist.launched - dist.failed;
                        if skipped > 0 { ui.label(egui::RichText::new(format!("{}: {}", t("accounts not distributed"), skipped)).small().color(theme.warning_text)); }
                        ui.add_space(8.0);
                        per_account_rows(ui, dist, accounts, anonymize);
                        ui.add_space(8.0);
                        if ui.button(t("Close")).clicked() { action = Some(GroupsAction::CloseDistribute); }
                    }
                }
            });
        });

    if !open { action = Some(GroupsAction::CloseDistribute); }
    action
}

fn per_account_rows(ui: &mut egui::Ui, dist: &DistributeState, accounts: &AccountStore, anonymize: bool) {
    let theme = ui.theme();
    let lang = ui.lang();
    let t = |key: &'static str| -> &'static str { i18n::tr(lang, key) };
    for status in &dist.statuses {
        let label = accounts.find_by_id(status.user_id)
            .map(|a| if anonymize { format!("Account {}", a.user_id) } else if a.alias.is_empty() { a.username.clone() } else { format!("{} ({})", a.alias, a.username) })
            .unwrap_or_else(|| status.user_id.to_string());
        let (color, text): (egui::Color32, String) = match status.state {
            DistAccountState::Waiting => (theme.text_muted, t("Waiting").to_string()),
            DistAccountState::Found => (theme.info, t("Server found").to_string()),
            DistAccountState::Entering => (theme.info, t("Entering").to_string()),
            DistAccountState::Done => (theme.success, format!("\u{2713} {}", t("Server found"))),
            DistAccountState::NoServer => (theme.warning, t("No server found for this account").to_string()),
            DistAccountState::AlreadyRunning => (theme.info, t("Already running").to_string()),
        };
        ui.horizontal(|ui| {
            ui.colored_label(color, format!("\u{25c9}"));
            ui.label(egui::RichText::new(label).size(13.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.colored_label(color, text); });
        });
    }
}