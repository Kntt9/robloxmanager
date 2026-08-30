//! Roblox REST API wrappers — avatar thumbnails, presence, place resolution.

use serde::Deserialize;

use crate::auth::RobloxClient;
use crate::error::CoreError;
use crate::models::{ModerationInfo, Presence};

// ---------------------------------------------------------------------------
// Avatar thumbnails
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ThumbnailResponse {
    data: Vec<ThumbnailEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThumbnailEntry {
    target_id: u64,
    image_url: Option<String>,
}

/// Fetch avatar headshot thumbnail URLs for a batch of user IDs.
/// Returns a vec of `(user_id, url)` pairs.
///
/// Deliberately unauthenticated: the thumbnails endpoint is public (same as
/// [`fetch_game_icons`]), and this is one batched call covering every account
/// at once. Signing it with some account's cookie meant a single revoked
/// cookie turned a would-be anonymous 200 into a 403, blanking the avatars
/// for every account in the list.
pub async fn fetch_avatars(
    client: &RobloxClient,
    user_ids: &[u64],
) -> Result<Vec<(u64, String)>, CoreError> {
    if user_ids.is_empty() {
        return Ok(vec![]);
    }
    let ids: Vec<String> = user_ids.iter().map(|id| id.to_string()).collect();
    let ids_param = ids.join(",");
    let url = format!(
        "https://thumbnails.roblox.com/v1/users/avatar-headshot\
         ?userIds={ids_param}&size=150x150&format=Png&isCircular=false"
    );

    let resp: ThumbnailResponse = client.get_json(&url, "").await?;
    Ok(pair_thumbnails(user_ids, resp.data))
}

/// Pair thumbnail entries back to the IDs that asked for them.
///
/// Keyed on `targetId`, never on position. The thumbnails service does not
/// promise request order, and it omits IDs it can't resolve (deleted accounts,
/// typo'd IDs) rather than returning a null entry. Both were observed live:
/// `/games/icons` echoes a reordered array, and `/users/avatar-headshot`
/// returns two entries for a three-ID request containing one bad ID. Zipping
/// the request against the response therefore handed images to the wrong
/// account or universe and dropped the trailing one.
fn pair_thumbnails(requested_ids: &[u64], data: Vec<ThumbnailEntry>) -> Vec<(u64, String)> {
    data.into_iter()
        .filter(|entry| requested_ids.contains(&entry.target_id))
        .filter_map(|entry| entry.image_url.map(|url| (entry.target_id, url)))
        .collect()
}

/// Download the actual image bytes for each avatar URL.
/// Returns a vec of `(user_id, raw_bytes)` pairs (skips failures).
///
/// Unauthenticated for the same reason as [`fetch_avatars`]: these URLs point
/// at the public `rbxcdn` image host, which has no use for a session cookie.
pub async fn download_avatar_images(
    client: &RobloxClient,
    avatars: &[(u64, String)],
) -> Vec<(u64, Vec<u8>)> {
    let mut results = Vec::new();
    for (id, url) in avatars {
        match client.get_bytes(url, "").await {
            Ok(bytes) => results.push((*id, bytes)),
            Err(e) => tracing::warn!("Failed to download avatar for {id}: {e}"),
        }
    }
    results
}

// ---------------------------------------------------------------------------
// Presence
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PresenceResponse {
    user_presences: Vec<PresenceEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PresenceEntry {
    user_presence_type: u8,
    place_id: Option<u64>,
    game_id: Option<String>,
    last_location: Option<String>,
}

/// Fetch presence info for multiple user IDs.
pub async fn fetch_presences(
    client: &RobloxClient,
    cookie: &str,
    user_ids: &[u64],
) -> Result<Vec<(u64, Presence)>, CoreError> {
    if user_ids.is_empty() {
        return Ok(vec![]);
    }
    let body = serde_json::json!({ "userIds": user_ids });
    let resp: PresenceResponse = client
        .post_json(
            "https://presence.roblox.com/v1/presence/users",
            cookie,
            Some(&body),
        )
        .await?;

    Ok(user_ids
        .iter()
        .zip(resp.user_presences.iter())
        .map(|(id, p)| {
            (
                *id,
                Presence {
                    user_presence_type: p.user_presence_type,
                    place_id: p.place_id,
                    game_id: p.game_id.clone(),
                    last_location: p.last_location.clone().unwrap_or_default(),
                },
            )
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Place / Universe resolution
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UniverseDetails {
    name: String,
}

#[derive(Deserialize)]
struct UniverseResponse {
    data: Vec<UniverseDetails>,
}

/// Resolve a universe ID to its game name. Works unauthenticated.
pub async fn resolve_universe_name(
    client: &RobloxClient,
    universe_id: u64,
) -> Result<String, CoreError> {
    let url = format!("https://games.roblox.com/v1/games?universeIds={universe_id}");
    let resp: UniverseResponse = client.get_json(&url, "").await?;
    resp.data
        .into_iter()
        .next()
        .map(|d| d.name)
        .ok_or_else(|| CoreError::RobloxApi {
            status: 404,
            message: format!("universe {universe_id} not found"),
        })
}

/// Result of resolving a place ID to a game preview: name plus thumbnail.
#[derive(Debug, Clone, Default)]
pub struct GamePreview {
    pub place_id: u64,
    pub universe_id: u64,
    pub name: String,
    pub thumb_url: String,
}

impl GamePreview {
    /// Builder-style setter so partial previews (e.g. from a failed lookup)
    /// can carry just the fields that were actually resolved.
    pub fn with_name(mut self, name: String) -> Self {
        self.name = name;
        self
    }
}

/// Resolve a place ID into its game (name + icon URL), without login.
///
/// Used by the Launch panel to confirm visually which game a Place ID points
/// at. Reuses the same place→universe resolution the private-server feature
/// uses, then the universe name/icon endpoints already in this module.
pub async fn resolve_game_preview(
    client: &RobloxClient,
    place_id: u64,
) -> Result<GamePreview, CoreError> {
    // Place → universe (public, no auth needed).
    let universe_url = format!(
        "https://apis.roblox.com/universes/v1/places/{place_id}/universe"
    );
    let universe_resp: serde_json::Value = client.get_json(&universe_url, "").await?;
    let universe_id = universe_resp
        .get("universeId")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| CoreError::RobloxApi {
            status: 404,
            message: format!("no universe found for place {place_id}"),
        })?;

    // Universe → name + icon (public).
    let name = resolve_universe_name(client, universe_id)
        .await
        .unwrap_or_default();
    let thumb_url = fetch_game_icons(client, "", &[universe_id])
        .await
        .map(|icons| {
            icons
                .into_iter()
                .next()
                .map(|(_, url)| url)
                .unwrap_or_default()
        })
        .unwrap_or_default();

    Ok(GamePreview {
        place_id,
        universe_id,
        name,
        thumb_url,
    })
}

/// Fetch game icon thumbnail URLs for a batch of universe IDs.
/// Returns a vec of `(universe_id, url)` pairs.
pub async fn fetch_game_icons(
    client: &RobloxClient,
    _cookie: &str,
    universe_ids: &[u64],
) -> Result<Vec<(u64, String)>, CoreError> {
    if universe_ids.is_empty() {
        return Ok(vec![]);
    }
    let ids: Vec<String> = universe_ids.iter().map(|id| id.to_string()).collect();
    let ids_param = ids.join(",");
    let url = format!(
        "https://thumbnails.roblox.com/v1/games/icons\
         ?universeIds={ids_param}&returnPolicy=PlaceHolder&size=150x150&format=Png&isCircular=false"
    );

    let resp: ThumbnailResponse = client.get_json(&url, "").await?;
    Ok(pair_thumbnails(universe_ids, resp.data))
}

// ---------------------------------------------------------------------------
// Server list (for Job ID joining + Servers panel)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameServer {
    /// The server's Job ID (GUID used to join this exact server).
    pub id: String,
    #[serde(default)]
    pub max_players: u32,
    #[serde(default)]
    pub playing: u32,
    #[serde(default)]
    pub fps: f32,
    /// Real ping in ms, when the API provides it. `None` = not reported.
    #[serde(default)]
    pub ping: Option<u32>,
}

impl GameServer {
    /// True when the server is at capacity and has no free slot.
    pub fn is_full(&self) -> bool {
        self.playing >= self.max_players
    }

    /// Free slots remaining. Never negative (playing can briefly exceed max).
    pub fn free_slots(&self) -> u32 {
        self.max_players.saturating_sub(self.playing)
    }
}

/// The servers response, parsed leniently: `data` may be missing/empty and
/// `nextPageCursor` may be absent. Missing optional fields must never fail the
/// whole page, or a single quirky server would hide every other.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerListResponse {
    #[serde(default)]
    data: Vec<GameServer>,
    #[serde(default)]
    next_page_cursor: Option<String>,
}

/// Fetch one page of public servers for a place.
///
/// `limit` must be one of the values the API accepts (10/25/50/100). The
/// cursor is URL-encoded before being appended (the API returns a base64-ish
/// token that can contain `+`, `/` and `=`, which must not be sent raw).
///
/// The body is read as text first and parsed manually so a non-JSON / error
/// page is reported as a clear error with diagnostics instead of a generic
/// "error decoding response body".
pub async fn fetch_servers(
    client: &RobloxClient,
    cookie: &str,
    place_id: u64,
    cursor: Option<&str>,
    limit: u32,
) -> Result<(Vec<GameServer>, Option<String>), CoreError> {
    let mut url = format!(
        "https://games.roblox.com/v1/games/{place_id}/servers/Public\
         ?sortOrder=Asc&limit={limit}"
    );
    if let Some(c) = cursor {
        // The cursor is a query value: percent-encode it.
        url.push_str(&format!("&cursor={}", percent_encode(c)));
    }
    tracing::debug!(
        "[Servers] GET place {place_id} cursor {}",
        cursor.unwrap_or("initial")
    );

    let text = client.get_text(&url, cookie).await?;
    let parsed: ServerListResponse = match serde_json::from_str(&text) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(
                "[Servers] place {place_id}: body not parseable ({} bytes): {e}",
                text.len()
            );
            tracing::debug!(
                "[Servers] place {place_id} body head: {}",
                text.chars().take(300).collect::<String>()
            );
            return Err(CoreError::Json(e));
        }
    };
    tracing::debug!(
        "[Servers] place {place_id}: parsed {} servers",
        parsed.data.len()
    );
    Ok((parsed.data, parsed.next_page_cursor))
}

/// Percent-encode a cursor value for safe inclusion in a URL query.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Share link resolution
// ---------------------------------------------------------------------------

/// Resolve a Roblox share link code (from `/share?code=CODE&type=Server`)
/// into `(place_id, universe_id, link_code, access_code)`.
///
/// Two-step process:
/// 1. POST `apis.roblox.com/sharelinks/v1/resolve-link` to get placeId + linkCode.
/// 2. GET `/games/{placeId}/game?privateServerLinkCode={linkCode}` and scrape
///    the UUID access code from the `joinPrivateGame(...)` JS call.
pub async fn resolve_share_link(
    client: &RobloxClient,
    cookie: &str,
    share_code: &str,
) -> Result<(u64, Option<u64>, String, String), CoreError> {
    use regex::Regex;

    // --- Step 1: Resolve share code → placeId + linkCode via API ---
    let body = serde_json::json!({
        "linkId": share_code,
        "linkType": "Server",
    });
    let resp: serde_json::Value = client
        .post_json(
            "https://apis.roblox.com/sharelinks/v1/resolve-link",
            cookie,
            Some(&body),
        )
        .await?;

    let ps_data = resp
        .get("privateServerInviteData")
        .ok_or_else(|| CoreError::RobloxApi {
            status: 400,
            message: "share link response missing privateServerInviteData".into(),
        })?;

    let place_id = ps_data
        .get("placeId")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| CoreError::RobloxApi {
            status: 400,
            message: "share link response missing placeId".into(),
        })?;

    let link_code = ps_data
        .get("linkCode")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CoreError::RobloxApi {
            status: 400,
            message: "share link response missing linkCode".into(),
        })?
        .to_string();

    let universe_id = ps_data.get("universeId").and_then(|v| v.as_u64());

    let status = ps_data
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");
    if status != "Valid" {
        return Err(CoreError::RobloxApi {
            status: 400,
            message: format!("private server invite status: {status}"),
        });
    }

    tracing::info!("Share link resolved → placeId={place_id}, linkCode={link_code}");

    // --- Step 2: Scrape accessCode (UUID) from the game page ---
    let game_url = format!(
        "https://www.roblox.com/games/{place_id}/game?privateServerLinkCode={link_code}"
    );
    let html = client.get_text(&game_url, cookie).await?;

    let access_re = Regex::new(
        r"Roblox\.GameLauncher\.joinPrivateGame\(\d+\s*,\s*'([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})'"
    ).expect("invalid regex");

    let access_code = access_re
        .captures(&html)
        .and_then(|cap| cap.get(1))
        .ok_or_else(|| CoreError::RobloxApi {
            status: 400,
            message: "could not scrape accessCode from game page".into(),
        })?
        .as_str()
        .to_string();

    tracing::info!("Access code resolved → {access_code}");

    Ok((place_id, universe_id, link_code, access_code))
}

// ---------------------------------------------------------------------------
// GitLab update check
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ReleaseLinks {
    #[serde(rename = "self")]
    self_url: String,
}

#[derive(Deserialize)]
struct GitLabRelease {
    tag_name: String,
    _links: ReleaseLinks,
}

/// Check for a newer release on GitLab. Returns `Some((version, url))` if an
/// update is available, `None` if already on the latest.
pub async fn check_for_updates(current_version: &str) -> Result<Option<(String, String)>, CoreError> {
    let client = reqwest::Client::builder()
        .user_agent("RM-update-check")
        .build()?;

    let release: GitLabRelease = client
        .get("https://gitlab.com/api/v4/projects/centerepic%2Frobloxmanager/releases/permalink/latest")
        .send()
        .await?
        .json()
        .await?;

    let remote = release.tag_name.trim_start_matches('v');
    let local = current_version.trim_start_matches('v');

    if remote != local {
        Ok(Some((remote.to_string(), release._links.self_url)))
    } else {
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// Moderation / enforcement detection
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublicUserResponse {
    #[serde(default)]
    is_banned: bool,
}

/// Check whether a Roblox user is **permanently terminated** via the public
/// profile endpoint. Works without a cookie. Temporary moderations are NOT
/// reflected here — use [`fetch_moderation_message`] alongside this for those.
pub async fn fetch_public_ban_status(
    client: &RobloxClient,
    user_id: u64,
) -> Result<bool, CoreError> {
    let url = format!("https://users.roblox.com/v1/users/{user_id}");
    let resp: PublicUserResponse = client.get_json(&url, "").await?;
    Ok(resp.is_banned)
}

#[derive(Deserialize)]
struct UsernameLookupResponse {
    data: Vec<UsernameLookupEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsernameLookupEntry {
    id: u64,
    name: String,
    display_name: String,
}

/// Look up a Roblox user by username. Returns `Ok(None)` if no such user
/// exists. Crucially, this passes `excludeBannedUsers: false` so the lookup
/// works for terminated accounts too — used by the "add anyway" flow when
/// the cookie itself has been revoked.
pub async fn lookup_username(
    client: &RobloxClient,
    username: &str,
) -> Result<Option<(u64, String, String)>, CoreError> {
    let body = serde_json::json!({
        "usernames": [username],
        "excludeBannedUsers": false,
    });
    let resp: UsernameLookupResponse = client
        .post_json(
            "https://users.roblox.com/v1/usernames/users",
            "",
            Some(&body),
        )
        .await?;
    Ok(resp
        .data
        .into_iter()
        .next()
        .map(|e| (e.id, e.name, e.display_name)))
}

/// v1 payload from `usermoderation.roblox.com/v1/not-approved`. Carries the
/// human-readable message and a punishment-type label. Fields we don't use
/// are intentionally left off.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct NotApprovedV1 {
    #[serde(default)]
    message_to_user: String,
    #[serde(default)]
    end_date: String,
}

/// v2 payload from `usermoderation.roblox.com/v2/not-approved`. Has the cleanest
/// machine-readable timestamps, so we use it for expiry resolution.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct NotApprovedV2 {
    restriction: Option<NotApprovedV2Restriction>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NotApprovedV2Restriction {
    #[serde(default)]
    end_time: Option<String>,
    #[serde(default)]
    duration_seconds: Option<i64>,
}

/// Cookie-only moderation probe. Hits the two `usermoderation.roblox.com`
/// endpoints (v1 for the localized message, v2 for the structured expiry).
/// Returns `(reason, expires_at)` when the cookie is recognised AND the
/// account is currently under an enforcement action, else `None`.
///
/// Doesn't need the user ID, so this works even when `validate_cookie` has
/// failed (e.g. on a terminated account whose cookie has been revoked enough
/// to break `users/authenticated` but not the moderation endpoints).
pub async fn fetch_moderation_message(
    client: &RobloxClient,
    cookie: &str,
) -> Option<(String, Option<chrono::DateTime<chrono::Utc>>)> {
    let v1: Option<NotApprovedV1> = client
        .get_json("https://usermoderation.roblox.com/v1/not-approved", cookie)
        .await
        .ok();
    let v2: Option<NotApprovedV2> = client
        .get_json("https://usermoderation.roblox.com/v2/not-approved", cookie)
        .await
        .ok();

    let reason = v1.as_ref().and_then(|p| {
        let m = p.message_to_user.trim();
        if m.is_empty() {
            None
        } else {
            Some(m.to_string())
        }
    })?;

    let expires_at = v2
        .as_ref()
        .and_then(|p| p.restriction.as_ref())
        .and_then(|r| {
            r.end_time
                .as_ref()
                .filter(|s| !s.is_empty())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.with_timezone(&chrono::Utc))
                .or_else(|| {
                    r.duration_seconds.and_then(|d| {
                        if d > 0 {
                            Some(chrono::Utc::now() + chrono::Duration::seconds(d))
                        } else {
                            None
                        }
                    })
                })
        })
        .or_else(|| {
            v1.as_ref().and_then(|p| {
                let s = p.end_date.trim();
                if s.is_empty() {
                    None
                } else {
                    chrono::DateTime::parse_from_rfc3339(s)
                        .ok()
                        .map(|d| d.with_timezone(&chrono::Utc))
                }
            })
        });
    Some((reason, expires_at))
}

/// Fetch the current moderation snapshot for the signed-in account, combining
/// the public `isBanned` check with the cookie-only moderation message.
pub async fn fetch_moderation_status(
    client: &RobloxClient,
    user_id: u64,
    cookie: &str,
) -> Result<Option<ModerationInfo>, CoreError> {
    let is_banned = fetch_public_ban_status(client, user_id)
        .await
        .unwrap_or(false);
    let msg = fetch_moderation_message(client, cookie).await;

    if !is_banned && msg.is_none() {
        return Ok(None);
    }

    let (reason, expires_at) = match msg {
        Some((r, e)) => (Some(r), e),
        None => (None, None),
    };

    Ok(Some(ModerationInfo {
        is_banned,
        // No fabricated fallback: leave `reason` as `None` when we don't have
        // a real one from the moderation endpoint. The UI can fall back to a
        // generic title and, crucially, the caller's merge logic can preserve
        // a previously-known specific reason instead of being clobbered by a
        // generic string on subsequent revalidations.
        reason,
        expires_at,
        last_checked: Some(chrono::Utc::now()),
    }))
}

// ---------------------------------------------------------------------------
// Popular games (discover / explore)
// ---------------------------------------------------------------------------

/// Response of `explore-api/v1/get-sort-content`. The interesting field is
/// `games`; the rest (sorts, layout, ads) is ignored.
#[derive(Deserialize)]
struct ExploreContentResponse {
    #[serde(default)]
    games: Vec<ExploreGame>,
}

/// One game in an explore sort, exactly as the discover page's own endpoint
/// reports it. `universeId` is the experience ID; `rootPlaceId` is what a
/// player actually joins and is the number RM users want to copy/launch.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExploreGame {
    universe_id: u64,
    root_place_id: u64,
    name: String,
    #[serde(default)]
    player_count: u64,
    /// The API reports this as `genreL1`; map it to the friendlier `genre`.
    #[serde(default, rename = "genreL1")]
    genre: String,
}

/// The explore feeds the Games tab offers, by the sort ID Roblox's own
/// discover page uses for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameSort {
    /// "Top Playing Now" — what people are in at this moment.
    Popular,
    /// "Top Rated" — highest player ratings.
    TopRated,
    /// "Top Earning" — highest revenue (a proxy for "most favorited overall").
    TopEarning,
}

impl GameSort {
    pub fn sort_id(self) -> &'static str {
        match self {
            GameSort::Popular => "top-playing-now",
            GameSort::TopRated => "top-rated",
            GameSort::TopEarning => "top-earning",
        }
    }

    /// Tab label in the Games tab.
    pub fn label(self) -> &'static str {
        match self {
            GameSort::Popular => "Popular",
            GameSort::TopRated => "Top Rated",
            GameSort::TopEarning => "Top Earning",
        }
    }
}

/// Fetch a batch of games from one explore sort, without any login.
///
/// The web Discover page loads these same feeds from
/// `apis.roblox.com/explore-api/v1/get-sort-content` with a fresh random
/// session ID per request; no cookie is involved. The old anonymous
/// `games.roblox.com/v1/games/list` endpoint is gone (404s), and the search /
/// discovery replacements require a session cookie, which is why this goes
/// straight to the endpoint the website itself uses.
///
/// Returns up to `limit` games (the endpoint currently hands back ~95–100).
pub async fn fetch_popular_games(
    client: &RobloxClient,
    sort: GameSort,
    limit: usize,
) -> Result<Vec<crate::models::PopularGame>, CoreError> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let url = format!(
        "https://apis.roblox.com/explore-api/v1/get-sort-content\
         ?sessionId={session_id}\
         &sortId={}\
         &device=computer\
         &country=all\
         &pageToken=",
        sort.sort_id()
    );
    let resp: ExploreContentResponse = client.get_json(&url, "").await?;

    let mut games = resp
        .games
        .into_iter()
        .take(limit)
        .map(|g| crate::models::PopularGame {
            universe_id: g.universe_id,
            root_place_id: g.root_place_id,
            name: g.name,
            player_count: g.player_count,
            genre: g.genre,
            thumb_url: String::new(),
        })
        .collect::<Vec<_>>();

    // Resolve icon URLs for every universe in the batch in one call.
    attach_thumbnails(client, &mut games).await;
    Ok(games)
}

/// Search Roblox for games by name, via the same omni-search the site's own
/// search bar hits. Returns matched games with resolved icons.
pub async fn search_games(
    client: &RobloxClient,
    query: &str,
    limit: usize,
) -> Result<Vec<crate::models::PopularGame>, CoreError> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let url = format!(
        "https://apis.roblox.com/search-api/omni-search\
         ?searchQuery={}\
         &sessionId={session_id}",
        urlencoding(query)
    );
    let resp: serde_json::Value = client.get_json(&url, "").await?;

    // Search results are grouped: find the section that is actually games.
    let results = resp
        .get("searchResults")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut universe_ids: Vec<u64> = Vec::new();
    for section in &results {
        let group_type = section.get("contentGroupType").and_then(|v| v.as_str());
        let is_game = group_type == Some("Game") || group_type.is_none();
        if !is_game {
            continue;
        }
        if let Some(contents) = section.get("contents").and_then(|v| v.as_array()) {
            for c in contents {
                let id = c.get("contentId").and_then(|v| v.as_u64());
                let kind = c.get("contentType").and_then(|v| v.as_str());
                if let Some(id) = id {
                    if kind == Some("Game") || kind.is_none() {
                        universe_ids.push(id);
                    }
                }
            }
        }
    }

    // Resolve names / root place IDs from the universe IDs.
    let mut games = resolve_universes(client, &universe_ids).await;
    games.truncate(limit);
    attach_thumbnails(client, &mut games).await;
    Ok(games)
}

/// URL-encode a query string the way serde_urlencoded / browsers do.
fn urlencoding(s: &str) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => {
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
}

/// Turn universe IDs into `PopularGame` rows (name, root place ID, player
/// count) via the public `games.roblox.com/v1/games?universeIds=` endpoint.
async fn resolve_universes(
    client: &RobloxClient,
    universe_ids: &[u64],
) -> Vec<crate::models::PopularGame> {
    if universe_ids.is_empty() {
        return Vec::new();
    }
    // One call covers all of them; the endpoint accepts a comma-separated list.
    let ids: String = universe_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let url = format!("https://games.roblox.com/v1/games?universeIds={ids}");
    let resp: serde_json::Value = match client.get_json(&url, "").await {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::with_capacity(universe_ids.len());
    if let Some(data) = resp.get("data").and_then(|v| v.as_array()) {
        for g in data {
            let universe_id = g.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let place_id = g
                .get("rootPlaceId")
                .and_then(|v| v.as_u64())
                .unwrap_or(universe_id);
            let name = g
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let playing = g.get("playing").and_then(|v| v.as_u64()).unwrap_or(0);
            let genre = g
                .get("genre")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            out.push(crate::models::PopularGame {
                universe_id,
                root_place_id: place_id,
                name,
                player_count: playing,
                genre,
                thumb_url: String::new(),
            });
        }
    }
    out
}

/// Fill `thumb_url` for every game in `games` in one thumbnails call.
async fn attach_thumbnails(
    client: &RobloxClient,
    games: &mut [crate::models::PopularGame],
) {
    if games.is_empty() {
        return;
    }
    let universe_ids: Vec<u64> = games.iter().map(|g| g.universe_id).collect();
    let icons = match fetch_game_icons(client, "", &universe_ids).await {
        Ok(icons) => icons,
        Err(e) => {
            tracing::warn!("Could not fetch game icons (non-fatal): {e}");
            return;
        }
    };
    let map: std::collections::HashMap<u64, String> = icons.into_iter().collect();
    for game in games.iter_mut() {
        if let Some(url) = map.get(&game.universe_id) {
            game.thumb_url = url.clone();
        }
    }
}

/// Download the thumbnail PNG for every game in `games` that has a `thumb_url`.
/// Returns `(universe_id, bytes)` pairs, skipping failures.
///
/// This mirrors [`download_avatar_images`]: egui's own HTTP loader is not
/// trusted to reach Roblox's CDN (it fails silently in the shipped build), so
/// the bytes are fetched on the backend thread and handed to the UI over the
/// event channel, which the UI then displays via its `bytes://` loader.
pub async fn download_game_thumbnails(
    client: &RobloxClient,
    games: &[crate::models::PopularGame],
) -> Vec<(u64, Vec<u8>)> {
    let mut results = Vec::new();
    for game in games {
        if game.thumb_url.is_empty() {
            continue;
        }
        match client.get_bytes(&game.thumb_url, "").await {
            Ok(bytes) => results.push((game.universe_id, bytes)),
            Err(e) => {
                tracing::warn!("Failed to download thumbnail for {}: {e}", game.universe_id);
            }
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(target_id: u64, image_url: Option<&str>) -> ThumbnailEntry {
        ThumbnailEntry {
            target_id,
            image_url: image_url.map(|s| s.to_string()),
        }
    }

    /// Roblox drops IDs it can't resolve rather than returning a null entry.
    /// Verified against the live endpoint: requesting `1,999999999999,156`
    /// returns two entries, for 1 and 156.
    #[test]
    fn missing_entries_do_not_shift_avatars_onto_wrong_accounts() {
        let requested = [1u64, 999_999_999_999, 156];
        let responded = vec![entry(1, Some("url-1")), entry(156, Some("url-156"))];

        let paired = pair_thumbnails(&requested, responded);

        assert_eq!(
            paired,
            vec![(1, "url-1".to_string()), (156, "url-156".to_string())]
        );
    }

    #[test]
    fn out_of_order_responses_pair_correctly() {
        let requested = [1u64, 156];
        let responded = vec![entry(156, Some("url-156")), entry(1, Some("url-1"))];

        let paired = pair_thumbnails(&requested, responded);

        assert_eq!(
            paired,
            vec![(156, "url-156".to_string()), (1, "url-1".to_string())]
        );
    }

    #[test]
    fn entries_without_an_image_are_skipped() {
        let requested = [1u64, 156];
        let responded = vec![entry(1, None), entry(156, Some("url-156"))];

        let paired = pair_thumbnails(&requested, responded);

        assert_eq!(paired, vec![(156, "url-156".to_string())]);
    }

    #[test]
    fn unrequested_ids_are_ignored() {
        let requested = [1u64];
        let responded = vec![entry(1, Some("url-1")), entry(999, Some("url-999"))];

        let paired = pair_thumbnails(&requested, responded);

        assert_eq!(paired, vec![(1, "url-1".to_string())]);
    }

    // -----------------------------------------------------------------------
    // Popular games (explore) parsing
    // -----------------------------------------------------------------------

    /// The exact shape the discover endpoint returns today: camelCase field
    /// names, `playerCount` as the live player number. Pinned so a re-name by
    /// Roblox shows up as a failing test rather than a runtime decode error.
    #[test]
    fn explore_content_parses_the_live_api_shape() {
        let body = r#"{
            "games": [
                {
                    "universeId": 10563114921,
                    "rootPlaceId": 107778070777162,
                    "name": "Steal An Egg",
                    "playerCount": 1640579,
                    "genreL1": "Simulation"
                },
                {
                    "universeId": 994732206,
                    "rootPlaceId": 2753915549,
                    "name": "Blox Fruits",
                    "playerCount": 458955,
                    "genreL1": "RPG"
                }
            ],
            "nextPageToken": ""
        }"#;

        let resp: ExploreContentResponse = serde_json::from_str(body).unwrap();
        assert_eq!(resp.games.len(), 2);

        let first = &resp.games[0];
        assert_eq!(first.universe_id, 10_563_114_921);
        assert_eq!(first.root_place_id, 107_778_070_777_162);
        assert_eq!(first.name, "Steal An Egg");
        assert_eq!(first.player_count, 1_640_579);
        assert_eq!(first.genre, "Simulation");

        let second = &resp.games[1];
        assert_eq!(second.root_place_id, 2_753_915_549);
        assert_eq!(second.player_count, 458_955);
        assert_eq!(second.genre, "RPG");
    }
}
