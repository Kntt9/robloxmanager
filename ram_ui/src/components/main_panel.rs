//! Main content panel — selected account details, avatar, launch controls.

use eframe::egui;
use ram_core::models::{Account, LaunchPreset};

use crate::i18n::LangUi;
use crate::theme::ThemeUi;

/// Actions the main panel can request.
pub enum MainPanelAction {
    LaunchGame { place_id: u64, job_id: Option<String> },
    RemoveAccount(u64),
    UpdateAlias { user_id: u64, alias: String },
    UpdateNotes { user_id: u64, notes: String },
    /// Persist the launch target (Place ID / Job ID) on this account so it
    /// survives a restart.
    SaveLaunchTarget { user_id: u64 },
    /// Save the current Place ID / Job ID inputs as a named launch preset.
    SavePreset {
        name: String,
        place_id: u64,
        job_id: Option<String>,
    },
    KillAll,
    /// Open a webview pre-logged in as this account.
    OpenBrowserAs(u64),
    /// Open the presets manager window.
    OpenPresets,
    /// Open the servers panel for the current Place ID.
    OpenServers,
}

/// Persistent input state for the main panel.
#[derive(Default)]
pub struct MainPanelState {
    pub place_id_input: String,
    pub job_id_input: String,
    /// Track which account the place/job inputs were loaded for, so switching
    /// accounts swaps in that account's saved launch target.
    launch_for_user: Option<u64>,
    pub alias_input: String,
    /// Track which account the alias input belongs to.
    alias_for_user: Option<u64>,
    /// Notes text buffer, keyed to the account it was loaded for.
    pub notes_input: String,
    notes_for_user: Option<u64>,
    /// Name buffer for the "Save as preset" inline form.
    pub preset_name_input: String,
    /// True while the "save as preset" popover is open.
    pub show_save_form: bool,
    /// Set the frame the save popover opens so we request focus exactly once.
    save_form_needs_focus: bool,
}

/// Result returned by the main panel.
pub struct MainPanelResult {
    pub action: Option<MainPanelAction>,
    /// Screen rect of the Launch button (for tutorial highlighting).
    pub launch_btn_rect: egui::Rect,
}

/// Draw the main panel for a selected account.
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut egui::Ui,
    account: &Account,
    state: &mut MainPanelState,
    roblox_running: bool,
    avatar_bytes: Option<&Vec<u8>>,
    presets: &[LaunchPreset],
    anonymize: bool,
    preview: Option<&ram_core::api::GamePreview>,
    preview_thumbs: Option<&Vec<u8>>,
) -> MainPanelResult {
    let theme = ui.theme();
    let mut action: Option<MainPanelAction> = None;
    let mut launch_btn_rect = egui::Rect::NOTHING;

    let lang = ui.lang();
    let t = |key: &'static str| -> &'static str { crate::i18n::tr(lang, key) };

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.vertical(|ui| {
            // -------------------------------------------------------------
            // Account header — a compact toolbar, not a giant card.
            // -------------------------------------------------------------
            egui::Frame::default()
                .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                .rounding(egui::Rounding::same(10.0))
                .fill(ui.visuals().extreme_bg_color)
                .show(ui, |ui: &mut egui::Ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        draw_avatar(ui, account.user_id, avatar_bytes, 52.0, anonymize);
                        ui.add_space(12.0);

                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                let name = if anonymize {
                                    "Account".to_string()
                                } else {
                                    account.display_name.clone()
                                };
                                ui.label(
                                    egui::RichText::new(name)
                                        .size(20.0)
                                        .strong()
                                        .color(ui.visuals().strong_text_color()),
                                );
                                ui.add_space(6.0);
                                draw_presence_chip(ui, &account.last_presence);
                            });
                            if !anonymize {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "@{}   \u{2022}   ID {}",
                                        account.username, account.user_id
                                    ))
                                    .size(13.0)
                                    .color(ui.visuals().weak_text_color()),
                                );
                            }
                        });

                        // Account actions on the right: Open browser, Presets,
                        // and the ⋮ menu with destructive actions.
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new("\u{1f310}").size(18.0),
                                        )
                                        .min_size(egui::vec2(36.0, 32.0)),
                                    )
                                    .on_hover_text(t("Open a webview signed in as this account"))
                                    .clicked()
                                {
                                    action = Some(MainPanelAction::OpenBrowserAs(
                                        account.user_id,
                                    ));
                                }
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new("\u{2b50}  Presets").size(15.0),
                                        )
                                        .min_size(egui::vec2(0.0, 32.0)),
                                    )
                                    .on_hover_text(t("Manage launch presets"))
                                    .clicked()
                                {
                                    action = Some(MainPanelAction::OpenPresets);
                                }
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(format!("\u{1f5a5}  {}", t("Servers"))).size(15.0),
                                        )
                                        .min_size(egui::vec2(0.0, 32.0)),
                                    )
                                    .on_hover_text(t("Open servers panel"))
                                    .clicked()
                                {
                                    action = Some(MainPanelAction::OpenServers);
                                }
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new("\u{1f5d1}").size(18.0),
                                        )
                                        .min_size(egui::vec2(36.0, 32.0)),
                                    )
                                    .on_hover_text(t("Move account to trash"))
                                    .clicked()
                                {
                                    action = Some(MainPanelAction::RemoveAccount(
                                        account.user_id,
                                    ));
                                }
                            },
                        );                    });
                });
            ui.add_space(10.0);

            // -------------------------------------------------------------
            // Moderation banner — most urgent info, surfaced before launch.
            // -------------------------------------------------------------
            if let Some(info) = account
                .moderation
                .as_ref()
                .filter(|m| m.is_active())
            {
                let banned = info.is_banned;
                let bg = if banned {
                    theme.danger_surface
                } else {
                    theme.warning_surface
                };
                let fg = if banned {
                    theme.danger_text
                } else {
                    theme.warning_text
                };
                egui::Frame::default()
                    .fill(bg)
                    .rounding(egui::Rounding::same(6.0))
                    .inner_margin(egui::Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.colored_label(
                                fg,
                                egui::RichText::new(if banned {
                                    "\u{26a0} Account terminated"
                                } else {
                                    "\u{26a0} Account moderated"
                                })
                                .strong()
                                .size(15.0),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .button("\u{1f310} Open browser as")
                                        .on_hover_text(
                                            "Sign in via webview to view the full moderation message or appeal",
                                        )
                                        .clicked()
                                    {
                                        action = Some(MainPanelAction::OpenBrowserAs(
                                            account.user_id,
                                        ));
                                    }
                                },
                            );
                        });
                        if let Some(reason) = &info.reason {
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new(reason).color(fg));
                        }
                        match &info.expires_at {
                            Some(exp) => {
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Expires: {}",
                                        exp.format("%Y-%m-%d %H:%M UTC")
                                    ))
                                    .small()
                                    .color(fg),
                                );
                            }
                            None if banned => {
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new("Permanent termination.")
                                        .small()
                                        .color(fg),
                                );
                            }
                            _ => {}
                        }
                        if let Some(checked) = &info.last_checked {
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new(format!(
                                    "Checked: {}",
                                    checked.format("%Y-%m-%d %H:%M UTC")
                                ))
                                .small()
                                .color(ui.visuals().weak_text_color()),
                            );
                        }
                    });
                ui.add_space(8.0);
            }

            // -------------------------------------------------------------
            // Launch — compact primary area. Two columns: destination inputs
            // on the left, the action button on the right.
            // -------------------------------------------------------------
            egui::Frame::default()
                .inner_margin(egui::Margin::same(14.0))
                .rounding(egui::Rounding::same(10.0))
                .fill(ui.visuals().extreme_bg_color)
                .show(ui, |ui: &mut egui::Ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(
                        egui::RichText::new(t("Launch"))
                            .strong()
                            .size(16.0)
                            .color(ui.visuals().strong_text_color()),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(t("Enter a place ID to launch this account"))
                            .small()
                            .color(ui.visuals().weak_text_color()),
                    );
                    ui.add_space(8.0);

                    // Load this account's saved launch target when it is selected,
                    // exactly like the alias/notes buffers above.
                    if state.launch_for_user != Some(account.user_id) {
                        state.place_id_input = account.saved_place_id.clone();
                        state.job_id_input = account.saved_job_id.clone();
                        state.launch_for_user = Some(account.user_id);
                    }

                    // Preset quick-select chips (compact).
                    if !presets.is_empty() {
                        let label = egui::RichText::new(t("Presets"))
                            .color(ui.visuals().weak_text_color());
                        if super::preset_chips(
                            ui,
                            label,
                            presets,
                            &mut state.place_id_input,
                            &mut state.job_id_input,
                        ) {
                            // A preset was just selected — persist the launch
                            // target so it survives a restart.
                            action = Some(MainPanelAction::SaveLaunchTarget {
                                user_id: account.user_id,
                            });
                        }
                        ui.add_space(8.0);
                    }

                    // Place / Job fields — full width, stacked. Nothing else
                    // shares their row, so they can never be overlapped.
                    let place_valid = state.place_id_input.parse::<u64>().is_ok();
                    let pid_resp =
                        labelled_input(ui, t("Place ID"), &mut state.place_id_input, "");
                    ui.add_space(6.0);
                    let jid_resp = labelled_input(
                        ui,
                        t("Job ID (optional)"),
                        &mut state.job_id_input,
                        t("Specific server GUID"),
                    );
                    if pid_resp.lost_focus() || jid_resp.lost_focus() {
                        action = Some(MainPanelAction::SaveLaunchTarget {
                            user_id: account.user_id,
                        });
                    }

                    // Game preview: a small confirmation of which game the
                    // Place ID points at. Only appears once resolved.
                    draw_game_preview(ui, state, preview, preview_thumbs);
                    ui.add_space(10.0);

                    // Action buttons on their own wrapped row, below the
                    // fields. `horizontal_wrapped` moves to the next line when
                    // there is not enough room, so the buttons can never sit on
                    // top of the inputs or of each other.
                    ui.horizontal_wrapped(|ui| {
                        let launch_btn = ui.add_enabled(
                            place_valid,
                            egui::Button::new(
                                egui::RichText::new("\u{1f680}  Launch")
                                    .size(15.0)
                                    .strong()
                                    .color(theme.on_accent),
                            )
                            .min_size(egui::vec2(110.0, 38.0))
                            .fill(if place_valid {
                                theme.accent
                            } else {
                                ui.visuals().widgets.inactive.bg_fill
                            }),
                        )
                        .on_hover_text(if place_valid {
                            t("Launch this account into the chosen place")
                        } else {
                            t("Enter a Place ID to launch")
                        });
                        launch_btn_rect = launch_btn.rect;
                        if launch_btn.clicked() {
                            if let Ok(place_id) = state.place_id_input.parse::<u64>() {
                                let job_id = parse_optional(&state.job_id_input);
                                action = Some(MainPanelAction::LaunchGame {
                                    place_id,
                                    job_id,
                                });
                            }
                        }
                        // Hover/active tint to make the primary obvious.
                        if launch_btn.hovered() && place_valid {
                            ui.painter().rect_filled(
                                launch_btn.rect,
                                egui::Rounding::same(3.0),
                                theme.accent_hover.linear_multiply(0.15),
                            );
                        }

                        ui.add_space(6.0);

                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("\u{1f310}  Open browser as")
                                        .size(15.0)
                                        .color(ui.visuals().strong_text_color()),
                                )
                                .min_size(egui::vec2(110.0, 38.0))
                                .fill(theme.surface_raised),
                            )
                            .on_hover_text("Open a webview signed in as this account")
                            .clicked()
                        {
                            action = Some(MainPanelAction::OpenBrowserAs(account.user_id));
                        }

                        if roblox_running
                            && ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("\u{2620}").size(15.0),
                                    )
                                    .min_size(egui::vec2(38.0, 38.0)),
                                )
                                .on_hover_text("Kill all running Roblox instances")
                                .clicked()
                        {
                            action = Some(MainPanelAction::KillAll);
                        }
                    });

                    // Save-as-preset button, small and unobtrusive below the
                    // action row.
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let save_resp = ui
                            .add_enabled(
                                place_valid,
                                egui::Button::new("\u{2b50}  Save as preset"),
                            )
                            .on_hover_text("Save these inputs as a launch preset");
                        if save_resp.clicked() {
                            state.show_save_form = !state.show_save_form;
                            if state.show_save_form {
                                state.preset_name_input.clear();
                                state.save_form_needs_focus = true;
                            }
                        }
                    });

                    // Inline save-as-preset popover (appears below the button
                    // when ⭐ is toggled).
                    if state.show_save_form {
                        ui.add_space(6.0);
                        egui::Frame::default()
                            .inner_margin(egui::Margin::same(8.0))
                            .rounding(egui::Rounding::same(4.0))
                            .fill(ui.visuals().faint_bg_color)
                            .stroke(egui::Stroke::new(
                                1.0,
                                ui.visuals().widgets.noninteractive.bg_stroke.color,
                            ))
                            .show(ui, |ui: &mut egui::Ui| {
                                ui.set_min_width(ui.available_width());
                                ui.label(egui::RichText::new("Save as preset").strong());
                                ui.add_space(4.0);
                                let txt_resp = ui.add(
                                    egui::TextEdit::singleline(&mut state.preset_name_input)
                                        .hint_text("Preset name")
                                        .desired_width(f32::INFINITY),
                                );
                                if state.save_form_needs_focus {
                                    txt_resp.request_focus();
                                    state.save_form_needs_focus = false;
                                }
                                let enter = txt_resp.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    let can_save = place_valid
                                        && !state.preset_name_input.trim().is_empty();
                                    let save_clicked = ui
                                        .add_enabled(can_save, egui::Button::new("Save"))
                                        .clicked();
                                    if (save_clicked || (enter && can_save)) && place_valid {
                                        if let Ok(pid) =
                                            state.place_id_input.parse::<u64>()
                                        {
                                            action = Some(MainPanelAction::SavePreset {
                                                name: state
                                                    .preset_name_input
                                                    .trim()
                                                    .to_string(),
                                                place_id: pid,
                                                job_id: parse_optional(&state.job_id_input),
                                            });
                                            state.preset_name_input.clear();
                                            state.show_save_form = false;
                                        }
                                    }
                                    if ui.button("Cancel").clicked() {
                                        state.show_save_form = false;
                                    }
                                });
                            });
                    }
                });
            ui.add_space(8.0);

            // -------------------------------------------------------------
            // Details + Notes — two columns so the info is easy to scan.
            // -------------------------------------------------------------
            ui.horizontal_top(|ui| {
                // ---- Details (left) ----
                egui::Frame::default()
                    .inner_margin(egui::Margin::same(14.0))
                    .rounding(egui::Rounding::same(10.0))
                    .fill(ui.visuals().extreme_bg_color)
                    .show(ui, |ui: &mut egui::Ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(
                            egui::RichText::new(t("Details"))
                                .strong()
                                .size(16.0)
                                .color(ui.visuals().strong_text_color()),
                        );
                        ui.add_space(8.0);

                        if state.alias_for_user != Some(account.user_id) {
                            state.alias_input = account.alias.clone();
                            state.alias_for_user = Some(account.user_id);
                        }

                        // Compact, scannable info grid — labels in the first
                        // column, values in the second, generous row spacing.
                        egui::Grid::new("meta_grid")
                            .num_columns(2)
                            .spacing([12.0, 8.0])
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(t("Alias"))
                                        .small()
                                        .color(ui.visuals().weak_text_color()),
                                );
                                let alias_response = ui.add(
                                    egui::TextEdit::singleline(&mut state.alias_input)
                                        .desired_width(160.0),
                                );
                                if alias_response.lost_focus()
                                    && state.alias_input != account.alias
                                {
                                    action = Some(MainPanelAction::UpdateAlias {
                                        user_id: account.user_id,
                                        alias: state.alias_input.clone(),
                                    });
                                }
                                ui.end_row();

                                if !account.group.is_empty() {
                                    ui.label(
                                        egui::RichText::new(t("Group"))
                                            .small()
                                            .color(ui.visuals().weak_text_color()),
                                    );
                                    ui.label(&account.group);
                                    ui.end_row();
                                }

                                if let Some(ts) = &account.last_validated {
                                    ui.label(
                                        egui::RichText::new(t("Validated"))
                                            .small()
                                            .color(ui.visuals().weak_text_color()),
                                    );
                                    let age = chrono::Utc::now() - *ts;
                                    let color = if age.num_hours() > 24 {
                                        theme.warning
                                    } else {
                                        ui.visuals().text_color()
                                    };
                                    ui.colored_label(
                                        color,
                                        ts.format("%Y-%m-%d %H:%M UTC").to_string(),
                                    );
                                    ui.end_row();
                                }

                                if !account.last_presence.last_location.is_empty() {
                                    ui.label(
                                        egui::RichText::new(t("Location"))
                                            .small()
                                            .color(ui.visuals().weak_text_color()),
                                    );
                                    ui.label(&account.last_presence.last_location);
                                    ui.end_row();
                                }
                            });

                        // Skip the cookie-expired banner when there's an active
                        // moderation — the moderation banner already covers it.
                        let mod_active = account
                            .moderation
                            .as_ref()
                            .is_some_and(|m| m.is_active());
                        if account.cookie_expired && !mod_active {
                            ui.add_space(8.0);
                            egui::Frame::default()
                                .fill(theme.danger_surface)
                                .rounding(egui::Rounding::same(6.0))
                                .inner_margin(8.0)
                                .show(ui, |ui| {
                                    ui.colored_label(
                                        theme.danger_text,
                                        "\u{26a0} Cookie expired. Remove and re-add this account with a fresh cookie.",
                                    );
                                });
                        }
                    });
            });

            ui.add_space(8.0);

            // -------------------------------------------------------------
            // Notes — a dedicated card with a header row, not a bare textarea.
            // -------------------------------------------------------------
            egui::Frame::default()
                .inner_margin(egui::Margin::same(14.0))
                .rounding(egui::Rounding::same(10.0))
                .fill(ui.visuals().extreme_bg_color)
                .show(ui, |ui: &mut egui::Ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(t("Notes"))
                                .strong()
                                .size(16.0)
                                .color(ui.visuals().strong_text_color()),
                        );
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.label(
                                    egui::RichText::new(t(
                                        "Saved automatically when you click away.",
                                    ))
                                    .small()
                                    .color(ui.visuals().weak_text_color()),
                                );
                            },
                        );
                    });
                    ui.add_space(6.0);

                    if state.notes_for_user != Some(account.user_id) {
                        state.notes_input = account.notes.clone();
                        state.notes_for_user = Some(account.user_id);
                    }

                    let notes_response = ui.add(
                        egui::TextEdit::multiline(&mut state.notes_input)
                            .desired_rows(4)
                            .desired_width(f32::INFINITY)
                            .hint_text(
                                t("Add notes about this account (origin, password hints, role\u{2026})"),
                            ),
                    );
                    if notes_response.lost_focus()
                        && state.notes_input != account.notes
                    {
                        action = Some(MainPanelAction::UpdateNotes {
                            user_id: account.user_id,
                            notes: state.notes_input.clone(),
                        });
                    }
                });
        });
    });

    MainPanelResult {
        action,
        launch_btn_rect,
    }
}

/// Show a placeholder when no account is selected.
pub fn show_empty(ui: &mut egui::Ui) {
    let lang = ui.lang();
    let t = |key: &'static str| -> &'static str { crate::i18n::tr(lang, key) };
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(
                egui::RichText::new("\u{1f4cb}")
                    .size(48.0)
                    .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(t("No account selected"))
                    .heading()
                    .color(ui.visuals().strong_text_color()),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(t("Pick an account in the sidebar to view it."))
                    .color(ui.visuals().weak_text_color()),
            );
        });
    });
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Render an account avatar from cached bytes or a placeholder of the same size.
/// `anonymize` only flips the URI discriminator so egui's image cache doesn't
/// serve the wrong variant when the toggle changes; the caller is responsible
/// for handing us the already-blurred bytes when anonymize is on.
fn draw_avatar(
    ui: &mut egui::Ui,
    user_id: u64,
    bytes: Option<&Vec<u8>>,
    size: f32,
    anonymize: bool,
) {
    let sz = egui::vec2(size, size);
    if let Some(bytes) = bytes {
        let variant = if anonymize { "anon" } else { "raw" };
        let uri = format!("bytes://avatar/{variant}_{user_id}.png");
        ui.add(
            egui::Image::from_bytes(uri, bytes.clone())
                .fit_to_exact_size(sz)
                .rounding(egui::Rounding::same(size / 8.0)),
        );
    } else {
        let (rect, _) = ui.allocate_exact_size(sz, egui::Sense::hover());
        ui.painter().rect_filled(
            rect,
            size / 8.0,
            ui.theme().surface,
        );
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "…",
            egui::FontId::proportional(size * 0.45),
            ui.theme().on_accent,
        );
    }
}

/// Pill-shaped presence chip ("Online" / "In game …" / "Offline" + colored dot).
fn draw_presence_chip(ui: &mut egui::Ui, presence: &ram_core::models::Presence) {
    let color = ui.theme().presence(presence.user_presence_type);
    let label = match presence.user_presence_type {
        1 => "Online",
        2 => "In game",
        3 => "In Studio",
        _ => "Offline",
    };
    let detail = presence.status_text();
    let text: String = if presence.user_presence_type == 0 || detail == label {
        label.to_string()
    } else {
        detail.to_string()
    };
    egui::Frame::default()
        .fill(color.linear_multiply(0.18))
        .stroke(egui::Stroke::new(1.0, color.linear_multiply(0.55)))
        .rounding(egui::Rounding::same(10.0))
        .inner_margin(egui::Margin::symmetric(8.0, 2.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (dot_rect, _) =
                    ui.allocate_exact_size(egui::vec2(8.0, 16.0), egui::Sense::hover());
                ui.painter().circle_filled(dot_rect.center(), 4.0, color);
                ui.label(egui::RichText::new(text).color(color).small());
            });
        });
}

/// Input with the label rendered above the field rather than to its left.
/// Returns the response so the caller can watch for lost focus.
fn labelled_input(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str) -> egui::Response {
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new(label)
                .color(ui.visuals().weak_text_color())
                .small(),
        );
        ui.add(
            egui::TextEdit::singleline(value)
                .desired_width(f32::INFINITY)
                .hint_text(hint),
        )
    })
    .inner
}

/// Trim and turn `""` into `None`, otherwise `Some(trimmed)`.
fn parse_optional(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// The visual confirmation of which game a Place ID points at, shown inside the
/// Launch card. States:
///
/// * no Place ID → nothing (or a discrete placeholder)
/// * Place ID set but not resolved yet → a light "loading" placeholder
/// * resolved with a name/icon → thumbnail + game name
/// * resolved but not found → a discrete "game not found" hint
fn draw_game_preview(
    ui: &mut egui::Ui,
    state: &MainPanelState,
    preview: Option<&ram_core::api::GamePreview>,
    preview_thumbs: Option<&Vec<u8>>,
) {
    let raw = state.place_id_input.trim();
    let Ok(current_place) = raw.parse::<u64>() else {
        return;
    };

    let preview_is_current = preview.is_some_and(|p| p.place_id == current_place);
    let thumbs_match = preview_is_current
        && preview_thumbs.is_some_and(|b| !b.is_empty());

    let lang = ui.lang();
    let t = |key: &'static str| -> &'static str { crate::i18n::tr(lang, key) };
    let theme = ui.theme();
    let thumb_size = egui::vec2(56.0, 56.0);

    ui.add_space(6.0);
    egui::Frame::default()
        .inner_margin(egui::Margin::same(8.0))
        .rounding(egui::Rounding::same(8.0))
        .fill(ui.visuals().faint_bg_color)
        .show(ui, |ui: &mut egui::Ui| {
            ui.horizontal(|ui| {
                if thumbs_match {
                    let uri = format!("bytes://game_preview/{current_place}");
                    ui.add(
                        egui::Image::from_bytes(uri, preview_thumbs.unwrap().clone())
                            .fit_to_exact_size(thumb_size)
                            .rounding(egui::Rounding::same(6.0)),
                    );
                } else {
                    let (rect, _) =
                        ui.allocate_exact_size(thumb_size, egui::Sense::hover());
                    ui.painter().rect_filled(
                        rect,
                        egui::Rounding::same(6.0),
                        theme.surface,
                    );
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "\u{1f3ae}",
                        egui::FontId::proportional(20.0),
                        theme.text_muted,
                    );
                }
                ui.add_space(10.0);

                ui.vertical(|ui| {
                    if preview_is_current {
                        if let Some(p) = preview {
                            if p.name.is_empty() {
                                ui.label(
                                    egui::RichText::new(t("Game not found for this Place ID."))
                                        .small()
                                        .color(theme.warning_text),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new(&p.name).strong().size(14.0),
                                );
                                ui.label(
                                    egui::RichText::new(t("Ready to launch"))
                                        .small()
                                        .color(ui.visuals().weak_text_color()),
                                );
                            }
                        }
                    } else {
                        ui.spinner();
                        ui.label(
                            egui::RichText::new(t("Identifying game\u{2026}"))
                                .small()
                                .color(ui.visuals().weak_text_color()),
                        );
                    }
                });
            });
        });
}
