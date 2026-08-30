//! Popular-games tab — three feed tabs (Popular, Top Rated, Top Earning),
//! plus search and one-click favorite. Combines Roblox's explore API with
//! the public thumbnails endpoint for a visual game browser.
//!
//! Thumbnails arrive as downloaded PNG bytes (see `ram_core::api::fetch_game_icons`
//! and the `attach_thumbnails` step in `fetch_popular_games`), keyed by
//! universe ID so egui can cache them by URL scheme.

use std::collections::HashMap;

use eframe::egui;
use ram_core::api::GameSort;
use ram_core::models::PopularGame;

use crate::i18n::{self, LangUi};
use crate::theme::ThemeUi;

/// The three feeds the Games tab offers. Kept in tab order.
pub const ALL_SORTS: [GameSort; 3] = [GameSort::Popular, GameSort::TopRated, GameSort::TopEarning];

/// Actions the games panel can request.
pub enum GamesAction {
    /// Re-fetch all three feeds.
    Refresh,
    /// Switch to a different sort tab.
    SetSort(GameSort),
    /// Search for games by name. Empty string clears the search.
    Search(String),
    /// Copy a game's Place ID to the clipboard.
    CopyPlaceId(u64),
    /// Toggle the favorite state for a universe ID.
    ToggleFavorite(u64),
}

/// Which sort tab the panel is showing. Lives here so it survives re-renders
/// and is not conflated with "the sort that happens to have data".
pub struct GamesState {
    pub active_sort: GameSort,
}

impl Default for GamesState {
    fn default() -> Self {
        Self {
            active_sort: GameSort::Popular,
        }
    }
}

impl GamesState {
    fn set_active(&mut self, sort: GameSort) {
        self.active_sort = sort;
    }
}

/// Draw the popular-games panel.
///
/// * `games` — cache of already-fetched feeds, keyed by sort.
/// * `errors` — per-sort fetch errors, if any.
/// * `thumbnails` — downloaded PNG bytes keyed by universe ID.
/// * `search` — `Some(result)` when a search is active, `None` for feed view.
/// * `search_error` — the last search error, if any.
/// * `search_input` — the current search text buffer.
/// * `favorites` — set of universe IDs the user has starred.
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut egui::Ui,
    state: &mut GamesState,
    games: &HashMap<GameSort, Vec<PopularGame>>,
    errors: &HashMap<GameSort, String>,
    thumbnails: &HashMap<u64, Vec<u8>>,
    search: &Option<Vec<PopularGame>>,
    search_error: Option<&str>,
    search_input: &str,
    favorites: &[u64],
) -> Option<GamesAction> {
    let theme = ui.theme();
    let lang = ui.lang();
    let t = |key: &'static str| -> &'static str { i18n::tr(lang, key) };
    let mut action: Option<GamesAction> = None;
    let fav_set: std::collections::HashSet<u64> = favorites.iter().copied().collect();

    egui::ScrollArea::vertical().show(ui, |ui| {
        // ---- Header ----
        ui.horizontal(|ui| {
            ui.heading(t("Games"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(format!("\u{1f504}  {}", t("Refresh")))
                    .on_hover_text(t("Re-fetch all game feeds"))
                    .clicked()
                {
                    action = Some(GamesAction::Refresh);
                }
            });
        });
        ui.add_space(4.0);

        // ---- Search bar ----
        let mut search_buf = search_input.to_string();
        let search_resp = ui.add(
            egui::TextEdit::singleline(&mut search_buf)
                .hint_text(t("Search games\u{2026}"))
                .desired_width(f32::INFINITY),
        );
        if search_resp.lost_focus() && search_buf != search_input {
            if search_buf.is_empty() {
                action = Some(GamesAction::Search(String::new()));
            } else {
                action = Some(GamesAction::Search(search_buf.clone()));
            }
        }

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(6.0);

        // ---- Search results override the feed view ----
        if let Some(search_results) = search {
            if search_results.is_empty() {
                ui.label(
                    egui::RichText::new(t("No games found for that search."))
                        .color(ui.visuals().weak_text_color()),
                );
                return;
            }
            let header = if search_results.len() == 1 {
                t("Search results ({} game)")
            } else {
                t("Search results ({} games)")
            };
            ui.label(
                egui::RichText::new(header.replace("{}", &search_results.len().to_string()))
                    .color(ui.visuals().weak_text_color()),
            );
ui.add_space(4.0);
            render_game_list(ui, search_results, thumbnails, &fav_set, &mut action, &theme, lang);
            return;
        }

        if let Some(err) = search_error {
            egui::Frame::default()
                .fill(theme.danger_surface)
                .rounding(egui::Rounding::same(6.0))
                .inner_margin(egui::Margin::same(10.0))
                .show(ui, |ui| {
                    ui.colored_label(theme.danger_text, format!("\u{26a0} {err}"));
                });
            return;
        }

        // ---- Sort tabs ----
        ui.horizontal(|ui| {
            for &sort in &ALL_SORTS {
                let label = sort_label(sort, lang);
                let selected = sort == state.active_sort;
                let btn = egui::Button::new(egui::RichText::new(label).strong());
                let resp = ui.add(if selected {
                    btn.fill(theme.accent).min_size(egui::vec2(0.0, 28.0))
                } else {
                    btn.min_size(egui::vec2(0.0, 28.0))
                });
                if resp.clicked() {
                    action = Some(GamesAction::SetSort(sort));
                    state.set_active(sort);
                }
            }
        });
        ui.add_space(6.0);

        // ---- Feed content ----
        let feed = games.get(&state.active_sort);
        let feed_err = errors.get(&state.active_sort);

        if let Some(err) = feed_err {
            egui::Frame::default()
                .fill(theme.danger_surface)
                .rounding(egui::Rounding::same(6.0))
                .inner_margin(egui::Margin::same(10.0))
                .show(ui, |ui| {
                    ui.colored_label(theme.danger_text, format!("\u{26a0} {err}"));
                    ui.add_space(4.0);
                    ui.colored_label(
                        theme.danger_text,
                        t("Check your connection, then try Refresh."),
                    );
                });
            return;
        }

        match feed {
            Some(list) if !list.is_empty() => {
                render_game_list(ui, list, thumbnails, &fav_set, &mut action, &theme, lang);
            }
            _ => {
                ui.label(
                    egui::RichText::new(t("Loading games\u{2026}"))
                        .color(ui.visuals().weak_text_color()),
                );
            }
        }
    });

    action
}

/// Render a list of game cards, each with thumbnail, name, stats, and action
/// buttons. Favorited games are shown first so starring something visibly
/// pins it to the top.
fn render_game_list(
    ui: &mut egui::Ui,
    games: &[PopularGame],
    thumbnails: &HashMap<u64, Vec<u8>>,
    favorites: &std::collections::HashSet<u64>,
    action: &mut Option<GamesAction>,
    theme: &crate::theme::Theme,
    lang: crate::i18n::Language,
) {
    // Stable partition: favorites float to the top, everything else keeps its
    // feed order underneath.
    let ordered: Vec<&PopularGame> = order_by_favorites(games, favorites);
    let t = |key: &'static str| -> &'static str { i18n::tr(lang, key) };

    let section_frame = egui::Frame::default()
        .inner_margin(egui::Margin::same(8.0))
        .rounding(egui::Rounding::same(6.0))
        .fill(ui.visuals().extreme_bg_color);

    for game in ordered {
        section_frame.show(ui, |ui: &mut egui::Ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                // ---- Thumbnail (bytes delivered by the backend) ----
                let thumb_size = egui::vec2(56.0, 56.0);
                match thumbnails.get(&game.universe_id) {
                    Some(bytes) if !bytes.is_empty() => {
                        let uri = format!("bytes://game_thumb/{}", game.universe_id);
                        ui.add(
                            egui::Image::from_bytes(uri, bytes.clone())
                                .fit_to_exact_size(thumb_size)
                                .rounding(egui::Rounding::same(6.0)),
                        );
                    }
                    _ => {
                        let (rect, _) = ui.allocate_exact_size(thumb_size, egui::Sense::hover());
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
                }
                ui.add_space(8.0);

                // ---- Info ----
                ui.vertical(|ui| {
                    ui.set_min_width(ui.available_width() - 130.0);
                    ui.label(egui::RichText::new(&game.name).strong().size(14.0));
                    let mut info = format!(
                        "\u{1f465} {}  \u{2022}  Place {}",
                        format_players(game.player_count),
                        game.root_place_id,
                    );
                    if !game.genre.is_empty() {
                        info.push_str(&format!("  \u{2022}  {}", game.genre));
                    }
                    ui.label(
                        egui::RichText::new(info)
                            .small()
                            .color(ui.visuals().weak_text_color()),
                    );
                });

                // ---- Action buttons ----
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        // Favorite star
                        let is_fav = favorites.contains(&game.universe_id);
                        let star = if is_fav { "\u{2b50}" } else { "\u{2606}" };
                        let star_color = if is_fav {
                            egui::Color32::from_rgb(255, 200, 50)
                        } else {
                            ui.visuals().weak_text_color()
                        };
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(star)
                                        .color(star_color)
                                        .size(16.0),
                                )
                                .min_size(egui::vec2(28.0, 28.0)),
                            )
                            .on_hover_text(if is_fav {
                                t("Remove from favorites")
                            } else {
                                t("Add to favorites")
                            })
                            .clicked()
                        {
                            *action = Some(GamesAction::ToggleFavorite(game.universe_id));
                        }

                        // Copy Place ID
                        if ui
                            .button("\u{1f4cb}")
                            .on_hover_text(t("Copy Place ID {}").replace("{}", &game.root_place_id.to_string()))
                            .clicked()
                        {
                            *action = Some(GamesAction::CopyPlaceId(game.root_place_id));
                        }
                    },
                );
            });
        });
        ui.add_space(4.0);
    }
}

/// Stable partition: games in `favorites` come first (in feed order), the rest
/// keep their feed order underneath. Starring a game therefore pins it to the
/// top of the current view.
fn order_by_favorites<'a>(
    games: &'a [PopularGame],
    favorites: &std::collections::HashSet<u64>,
) -> Vec<&'a PopularGame> {
    let mut ordered = Vec::with_capacity(games.len());
    ordered.extend(games.iter().filter(|g| favorites.contains(&g.universe_id)));
    ordered.extend(games.iter().filter(|g| !favorites.contains(&g.universe_id)));
    ordered
}

/// Format a player count compactly: 1,640,579 -> "1.64M".
fn format_players(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Human-readable label for each sort, translated.
fn sort_label(sort: GameSort, lang: crate::i18n::Language) -> String {
    match sort {
        GameSort::Popular => i18n::tr(lang, "Popular"),
        GameSort::TopRated => i18n::tr(lang, "Top Rated"),
        GameSort::TopEarning => i18n::tr(lang, "Top Earning"),
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_counts_format_compactly() {
        assert_eq!(format_players(0), "0");
        assert_eq!(format_players(999), "999");
        assert_eq!(format_players(1_000), "1.0K");
        assert_eq!(format_players(552_454), "552.5K");
        assert_eq!(format_players(1_640_579), "1.64M");
    }

    #[test]
    fn each_feed_has_a_distinct_sort_id() {
        let ids: Vec<&str> = ALL_SORTS.iter().map(|s| s.sort_id()).collect();
        let mut dedup = ids.clone();
        dedup.dedup();
        assert_eq!(ids.len(), dedup.len(), "duplicate sort id");
    }

    fn game(universe: u64, name: &str) -> PopularGame {
        PopularGame {
            universe_id: universe,
            root_place_id: universe + 1,
            name: name.to_string(),
            player_count: 10,
            genre: String::new(),
            thumb_url: String::new(),
        }
    }

    #[test]
    fn favorites_float_to_the_top_and_keep_feed_order() {
        let games = vec![
            game(1, "A"),
            game(2, "B"),
            game(3, "C"),
            game(4, "D"),
        ];
        let favs: std::collections::HashSet<u64> = [3u64, 1].into_iter().collect();

        let ordered = order_by_favorites(&games, &favs);
        let names: Vec<&str> = ordered.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, vec!["A", "C", "B", "D"]);
    }

    #[test]
    fn with_no_favorites_order_is_unchanged() {
        let games = vec![game(1, "A"), game(2, "B")];
        let favs = std::collections::HashSet::new();
        let ordered = order_by_favorites(&games, &favs);
        let names: Vec<&str> = ordered.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, vec!["A", "B"]);
    }
}