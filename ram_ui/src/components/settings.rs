//! Settings panel — global config, encryption toggles, multi-instance control.

use eframe::egui;
use ram_core::models::AppConfig;

use crate::i18n::{self, LangUi};
use crate::theme::ThemeUi;

/// Actions the settings panel can emit.
#[allow(dead_code)]
pub enum SettingsAction {
    SaveConfig,
    ChangePassword { new_password: String },
    ClearPassword,
    EnableMultiInstance,
    DisableMultiInstance,
    /// Export all accounts (with decrypted cookies) to a file.
    ExportAccounts,
    /// Import accounts from an export file.
    ImportAccounts,
    /// User changed the UI language.
    LanguageChanged { language: crate::i18n::Language },
}

/// Persistent state for the settings panel password change UI.
#[derive(Default)]
pub struct SettingsState {
    pub new_password_input: String,
    pub confirm_password_input: String,
}

/// Draw the settings UI. Returns `Some(SettingsAction)` when an action is triggered.
pub fn show(
    ui: &mut egui::Ui,
    config: &mut AppConfig,
    has_password: bool,
    settings_state: &mut SettingsState,
    roblox_running: bool,
) -> Option<SettingsAction> {
    let theme = ui.theme();
    let lang = ui.lang();
    let t = |key: &'static str| -> &'static str { i18n::tr(lang, key) };
    let mut action: Option<SettingsAction> = None;

    egui::ScrollArea::vertical().show(ui, |ui| {

    ui.heading(t("Settings"));
    ui.separator();
    ui.add_space(8.0);

    let section_frame = egui::Frame::default()
        .inner_margin(egui::Margin::same(10.0))
        .rounding(egui::Rounding::same(6.0))
        .fill(ui.visuals().extreme_bg_color);

    // ---- Storage backend ----
    section_frame.show(ui, |ui: &mut egui::Ui| {
        ui.set_min_width(ui.available_width());
        ui.strong(t("Storage"));
        ui.add_space(4.0);
        ui.checkbox(
            &mut config.use_credential_manager,
            t("Use Windows Credential Manager (instead of encrypted file)"),
        );
    });
    ui.add_space(6.0);

    // ---- Launch Behavior ----
    section_frame.show(ui, |ui: &mut egui::Ui| {
        ui.set_min_width(ui.available_width());
        ui.strong(t("Launch Behavior"));
        ui.add_space(4.0);

        let mut wants_multi = config.multi_instance_enabled;
        let toggled = ui.checkbox(
            &mut wants_multi,
            t("Enable multi-instance"),
        ).changed();
        if toggled {
            if wants_multi {
                action = Some(SettingsAction::EnableMultiInstance);
            } else {
                action = Some(SettingsAction::DisableMultiInstance);
            }
        }
        if config.multi_instance_enabled {
            ui.colored_label(
                theme.warning,
                t("\u{26a0} This interacts with Hyperion anti-cheat and may carry ban risk."),
            );
        }
        if !config.multi_instance_enabled && roblox_running {
            ui.colored_label(
                theme.text_muted,
                t("Close all Roblox processes (including tray) before enabling."),
            );
        }

        ui.add_space(4.0);
        ui.checkbox(
            &mut config.kill_background_roblox,
            t("Kill Roblox tray/background processes automatically"),
        ).on_hover_text(t("Kills idle \"always running\" Roblox processes (--launch-to-tray)."));
        if config.multi_instance_enabled && !config.kill_background_roblox {
            ui.colored_label(
                theme.warning,
                t("\u{26a0} Recommended when multi-instance is enabled. Tray processes stack up."),
            );
        }

        ui.add_space(4.0);
        ui.checkbox(
            &mut config.auto_arrange_windows,
            t("Auto-arrange Roblox windows after launch"),
        ).on_hover_text(t("Tiles Roblox windows in a grid (2 = side-by-side, 4 = 2×2, etc.)."));

        ui.add_space(4.0);
        ui.checkbox(
            &mut config.rename_roblox_windows,
            t("Name Roblox windows after their account"),
        ).on_hover_text(
            t("Renames each launched Roblox window to the account's alias, so tiled windows are tellable apart.\n\nOff by default. This writes to the Roblox window rather than just reading it, and how Hyperion treats that is not documented. It also changes what capture software matching on window title will find."),
        );
        if config.rename_roblox_windows && !config.anonymize_names {
            ui.colored_label(
                theme.text_muted,
                t("Window titles are readable by any program, and show up in screenshots and streams."),
            );
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(t("Launch delay:"));
            let mut secs = config.launch_delay_secs as i32;
            ui.add(
                egui::DragValue::new(&mut secs)
                    .range(0..=300)
                    .speed(0.2)
                    .suffix(" s"),
            )
            .on_hover_text(
                "Minimum gap between account launches. Applies to single and bulk launches. 0 disables throttling.",
            );
            config.launch_delay_secs = secs.max(0) as u32;
            ui.label(
                egui::RichText::new("(Roblox rate-limits some IPs)")
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
        });
    });
    ui.add_space(6.0);

    // ---- Privacy ----
    section_frame.show(ui, |ui: &mut egui::Ui| {
        ui.set_min_width(ui.available_width());
        ui.strong(t("Privacy"));
        ui.add_space(4.0);
        ui.checkbox(
            &mut config.privacy_mode,
            t("Clear RobloxCookies.dat before each launch"),
        ).on_hover_text(t("Prevents Roblox from associating your accounts via stored cookies."));
        ui.checkbox(
            &mut config.anonymize_names,
            t("Anonymize account names"),
        ).on_hover_text(t("Replaces usernames and display names with generic \"Account 1\", \"Account 2\", etc."));
    });
    ui.add_space(6.0);

    // ---- Developer options ----
    section_frame.show(ui, |ui: &mut egui::Ui| {
        ui.set_min_width(ui.available_width());
        ui.strong(t("Developer Options"));
        ui.add_space(4.0);
        ui.checkbox(
            &mut config.developer_options,
            t("Show the Asset Manager tab"),
        ).on_hover_text(
            t("Upload assets to Roblox from any saved account, track moderation, and grant experiences permission to use them."),
        );
        if config.developer_options {
            ui.colored_label(
                theme.warning,
                t("\u{26a0} Uploads are permanent and public. Every asset is moderated under the account that uploaded it."),
            );
        }
    });
    ui.add_space(6.0);

    // ---- Roblox path override ----
    section_frame.show(ui, |ui: &mut egui::Ui| {
        ui.set_min_width(ui.available_width());
        ui.strong(t("Roblox Player Path"));
        ui.add_space(4.0);
        ui.label(t("Leave empty for auto-detect:"));
        let mut path_str = config
            .roblox_player_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if ui.text_edit_singleline(&mut path_str).changed() {
            config.roblox_player_path = if path_str.trim().is_empty() {
                None
            } else {
                Some(std::path::PathBuf::from(path_str))
            };
        }
    });

    ui.add_space(6.0);

    // ---- Backup & Transfer ----
    section_frame.show(ui, |ui: &mut egui::Ui| {
        ui.set_min_width(ui.available_width());
        ui.strong("Backup & Transfer");
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "Export your accounts to a portable file. Cookies are included in \
                 plaintext, so treat the file like a password manager backup."
            )
            .small()
            .weak(),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button("\u{1f4e4}  Export accounts...").clicked() {
                action = Some(SettingsAction::ExportAccounts);
            }
            if ui.button("\u{1f4e5}  Import accounts...").clicked() {
                action = Some(SettingsAction::ImportAccounts);
            }
        });
    });
    ui.add_space(6.0);

    // ---- Language ----
    section_frame.show(ui, |ui: &mut egui::Ui| {
        ui.set_min_width(ui.available_width());
        ui.strong("Language");
        ui.add_space(4.0);
        let current_lang = crate::i18n::of(ui.ctx());
        let mut new_lang = current_lang;
        egui::ComboBox::from_id_salt("lang_picker")
            .selected_text(current_lang.label())
            .show_ui(ui, |ui| {
                for lang in &[crate::i18n::Language::En, crate::i18n::Language::PtBr] {
                    ui.selectable_value(&mut new_lang, *lang, lang.label());
                }
            });
        if new_lang != current_lang {
            action = Some(SettingsAction::LanguageChanged { language: new_lang });
        }
    });

    ui.add_space(12.0);

    if ui.button(format!("\u{1f4be}  {}", t("Save Settings"))).clicked() {
        action = Some(SettingsAction::SaveConfig);
    }

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    // ---- Encryption ----
    section_frame.show(ui, |ui: &mut egui::Ui| {
        ui.set_min_width(ui.available_width());
        ui.strong(t("Encryption"));
        ui.add_space(4.0);

        if has_password {
            ui.label(t("Accounts are encrypted with your master password."));
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new(
                    t("KNT Manager asks for it every time it starts. If you forget it, the accounts cannot be recovered."),
                )
                .small()
                .weak(),
            );
        } else {
            ui.label(t("Accounts are encrypted and unlock automatically on this PC."));
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new(
                    t("The key is held in Windows Credential Manager, so the file is useless on its own. Anything running as you can still read it."),
                )
                .small()
                .weak(),
            );
        }

        ui.add_space(10.0);
        ui.label(if has_password {
            t("Change your master password:")
        } else {
            t("Require a master password at startup:")
        });
        ui.add_space(4.0);

        ui.add(
            egui::TextEdit::singleline(&mut settings_state.new_password_input)
                .password(true)
                .hint_text(t("New password")),
        );
        ui.add(
            egui::TextEdit::singleline(&mut settings_state.confirm_password_input)
                .password(true)
                .hint_text(t("Confirm password")),
        );
        ui.add_space(4.0);

        let passwords_match = !settings_state.new_password_input.is_empty()
            && settings_state.new_password_input == settings_state.confirm_password_input;

        if !settings_state.new_password_input.is_empty()
            && !settings_state.confirm_password_input.is_empty()
            && !passwords_match
        {
            ui.colored_label(
                theme.danger,
                t("Passwords do not match."),
            );
        }

        ui.horizontal(|ui| {
            let label = if has_password {
                format!("\u{1f511}  {}", t("Change password"))
            } else {
                format!("\u{1f511}  {}", t("Set password"))
            };
            if ui
                .add_enabled(passwords_match, egui::Button::new(label))
                .clicked()
            {
                let new_pw = settings_state.new_password_input.clone();
                settings_state.new_password_input.clear();
                settings_state.confirm_password_input.clear();
                action = Some(SettingsAction::ChangePassword {
                    new_password: new_pw,
                });
            }

            // Only offered to someone who has a password to remove. The store
            // stays encrypted either way, so this is a convenience toggle
            // rather than a way to turn encryption off.
            if has_password && ui.button(t("Stop asking for a password")).clicked() {
                settings_state.new_password_input.clear();
                settings_state.confirm_password_input.clear();
                action = Some(SettingsAction::ClearPassword);
            }
        });
    });

    }); // ScrollArea

    action
}
