use crate::config;
use crate::error::CommandResult;
use chrono::{DateTime, Local, LocalResult, TimeZone, Utc};
use image::{codecs::png::PngEncoder, imageops, ColorType, ImageEncoder, ImageFormat};
use reqwest::StatusCode;
use serde::Deserialize;
use serenity::client::Context;
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOption, CommandDataOptionValue,
};
use serenity::model::channel::AttachmentType;
use serenity::utils::Colour;
use std::borrow::Cow;
use std::fmt;
use std::time::{Duration, SystemTime};
use tracing::{error, warn};

const CLASS_COLOURS: [Colour; 12] = [
    Colour::from_rgb(199, 156, 110),
    Colour::from_rgb(245, 140, 186),
    Colour::from_rgb(169, 210, 113),
    Colour::from_rgb(255, 245, 105),
    Colour::from_rgb(255, 255, 255),
    Colour::from_rgb(196, 31, 59),
    Colour::from_rgb(0, 112, 222),
    Colour::from_rgb(64, 199, 235),
    Colour::from_rgb(135, 135, 237),
    Colour::from_rgb(0, 255, 150),
    Colour::from_rgb(255, 125, 10),
    Colour::from_rgb(163, 48, 201),
];

#[derive(Deserialize)]
struct AuthResponse {
    access_token: String,
    expires_in: u64,
}

#[derive(Deserialize)]
struct KeyValue {
    key: String,
    value: String,
}

#[derive(Deserialize)]
struct Stat {
    effective: u32,
}

impl fmt::Display for Stat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.effective)
    }
}

#[derive(Deserialize)]
struct Rating {
    value: f32,
}

impl fmt::Display for Rating {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:.2}%", self.value)
    }
}

#[derive(Deserialize)]
struct Name {
    name: String,
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Deserialize)]
struct IdName {
    id: u32,
    name: String,
}

impl fmt::Display for IdName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Deserialize)]
struct TypeName {
    #[serde(rename = "type")]
    t: String,
    name: String,
}

impl fmt::Display for TypeName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Deserialize)]
struct DisplayString {
    display_string: String,
}

#[derive(Deserialize)]
struct Covenant {
    name: String,
}

#[derive(Deserialize)]
struct CovenantProgress {
    chosen_covenant: Covenant,
    renown_level: u32,
}

#[derive(Deserialize)]
struct CharacterMedia {
    assets: Option<Vec<KeyValue>>,
    render_url: Option<String>,
    #[serde(skip)]
    last_modified: Option<DateTime<Local>>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct Character {
    id: u64,
    name: String,
    gender: TypeName,
    faction: Name,
    race: IdName,
    character_class: IdName,
    active_spec: Option<Name>,
    realm: Name,
    guild: Option<Name>,
    level: u32,
    achievement_points: u32,
    last_login_timestamp: i64,
    average_item_level: u32,
    equipped_item_level: u32,
    covenant_progress: Option<CovenantProgress>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct CharacterStats {
    health: u64,
    power: u32,
    power_type: Name,
    strength: Stat,
    agility: Stat,
    intellect: Stat,
    stamina: Stat,
    armor: Stat,
    melee_crit: Rating,
    melee_haste: Rating,
    ranged_crit: Rating,
    ranged_haste: Rating,
    spell_crit: Rating,
    spell_haste: Rating,
    mastery: Rating,
    versatility_damage_done_bonus: f32,
}

#[derive(Deserialize)]
struct CharacterTitles {
    active_title: Option<DisplayString>,
}

impl CharacterStats {
    fn crit(&self) -> f32 {
        self.melee_crit
            .value
            .max(self.ranged_crit.value.max(self.spell_crit.value))
    }

    fn haste(&self) -> f32 {
        self.melee_haste
            .value
            .max(self.ranged_haste.value.max(self.spell_haste.value))
    }
}

impl Character {
    fn last_login_local(&self) -> Option<DateTime<Local>> {
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        match Local.timestamp_opt(
            self.last_login_timestamp / 1000,
            ((self.last_login_timestamp % 1000) * 1000) as u32,
        ) {
            LocalResult::None => None,
            LocalResult::Single(t) | LocalResult::Ambiguous(t, _) => Some(t),
        }
    }

    fn last_login_utc(&self) -> Option<DateTime<Utc>> {
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        match Utc.timestamp_opt(
            self.last_login_timestamp / 1000,
            ((self.last_login_timestamp % 1000) * 1000) as u32,
        ) {
            LocalResult::None => None,
            LocalResult::Single(t) | LocalResult::Ambiguous(t, _) => Some(t),
        }
    }
}

#[derive(Deserialize)]
struct StatusType {
    #[serde(rename = "type")]
    t: String,
}

#[derive(Deserialize)]
struct Localized {
    #[serde(rename = "en_US")]
    en_us: Option<String>,
}

#[derive(Deserialize)]
struct Realm {
    name: Localized,
    slug: String,
}

#[derive(Deserialize)]
struct SearchData {
    has_queue: bool,
    realms: Vec<Realm>,
    status: StatusType,
}

#[derive(Deserialize)]
struct SearchResult {
    data: SearchData,
}

#[derive(Deserialize)]
struct Search {
    results: Vec<SearchResult>,
}

// Get access token from global state or Blizzard API if token missing/expired
async fn get_access_token(ctx: &Context) -> Result<String, reqwest::Error> {
    let mut wow_config = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        data.get::<config::Wow>().unwrap().clone()
    };
    // If we already have auth and it hasn't expired yet
    if let Some(auth) = wow_config.auth.and_then(|a| {
        if a.expires_at > SystemTime::now() {
            Some(a)
        } else {
            None
        }
    }) {
        // Return existing saved token
        Ok(auth.access_token)
    } else {
        // Otherwise, fetch new access token since we currently have no auth info or auth has expired
        let new_auth = auth(&wow_config.client_id, &wow_config.client_secret).await?;
        let access_token = new_auth.access_token.clone();
        let mut data = ctx.data.write().await;
        wow_config.auth = Some(new_auth);
        data.insert::<config::Wow>(wow_config);
        Ok(access_token)
    }
}

async fn get_character(
    realm_name: &str,
    character_name: &str,
    access_token: &str,
) -> Result<Character, reqwest::Error> {
    let client = reqwest::Client::new();

    // Get character last login time (and check if they exist)
    let resp = client.get(format!("https://us.api.blizzard.com/profile/wow/character/{realm_name}/{character_name}?namespace=profile-us&locale=en_US&access_token={access_token}"))
        .send().await?;
    match resp.error_for_status() {
        Ok(resp) => Ok(resp.json::<Character>().await?),
        Err(e) => Err(e),
    }
}

async fn get_character_media(
    realm_name: &str,
    character_name: &str,
    access_token: &str,
    race_id: Option<u32>,
    gender_type: Option<&str>,
) -> Result<CharacterMedia, reqwest::Error> {
    let client = reqwest::Client::new();

    #[allow(clippy::unwrap_used)]
    let alt_avatar = if race_id.is_some() && gender_type.is_some() {
        format!(
            "&alt=/shadow/avatar/{}-{}.jpg",
            race_id.unwrap(),
            i32::from(gender_type.unwrap() != "MALE")
        )
    } else {
        String::new()
    };

    // Get JSON info of character's appearance and last modified time of images
    let resp = client.get(&format!("https://us.api.blizzard.com/profile/wow/character/{realm_name}/{character_name}/character-media?namespace=profile-us&locale=en_US&access_token={access_token}{alt_avatar}"))
        .send().await?;
    match resp.error_for_status() {
        Ok(resp) => {
            let last_modified: Option<DateTime<Local>> = match &resp.headers().get("last-modified")
            {
                Some(lm) => match lm.to_str() {
                    Ok(val) => {
                        if let Ok(last_modified) = DateTime::parse_from_rfc2822(val) {
                            Some(last_modified.with_timezone(&Local))
                        } else {
                            warn!(last_modified = val, "Unable to parse last-modified");
                            None
                        }
                    }
                    Err(_) => None,
                },
                None => None,
            };
            let mut media = resp.json::<CharacterMedia>().await?;
            media.last_modified = last_modified;
            Ok(media)
        }
        Err(e) => Err(e),
    }
}

async fn get_character_statistics(
    realm_name: &str,
    character_name: &str,
    access_token: &str,
) -> Result<CharacterStats, reqwest::Error> {
    let client = reqwest::Client::new();

    let resp = client.get(&format!("https://us.api.blizzard.com/profile/wow/character/{realm_name}/{character_name}/statistics?namespace=profile-us&locale=en_US&access_token={access_token}"))
        .send().await?;
    match resp.error_for_status() {
        Ok(resp) => Ok(resp.json::<CharacterStats>().await?),
        Err(e) => Err(e),
    }
}

async fn get_character_titles(
    realm_name: &str,
    character_name: &str,
    access_token: &str,
) -> Result<CharacterTitles, reqwest::Error> {
    let client = reqwest::Client::new();

    let resp = client.get(&format!("https://us.api.blizzard.com/profile/wow/character/{realm_name}/{character_name}/titles?namespace=profile-us&locale=en_US&access_token={access_token}"))
        .send().await?;
    match resp.error_for_status() {
        Ok(resp) => Ok(resp.json::<CharacterTitles>().await?),
        Err(e) => Err(e),
    }
}

// Get bearer auth token from Blizzard API with client_id and client_secret
async fn auth(client_id: &str, client_secret: &str) -> Result<config::WowAuth, reqwest::Error> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://us.battle.net/oauth/token")
        .basic_auth(client_id, Some(client_secret))
        .form(&[("grant_type", "client_credentials")])
        .send()
        .await?
        .json::<AuthResponse>()
        .await?;

    // Mark auth as expiring a little early so that we refresh before absolutely necessary
    let expires_at = SystemTime::now() + Duration::from_secs((resp.expires_in - 60).max(0));

    Ok(config::WowAuth {
        access_token: resp.access_token,
        expires_at,
    })
}

// Tries to get transparency png image and crop it, otherwise returns "deafult" jpg image with background
pub async fn transmog(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    options: &Vec<CommandDataOption>,
) -> CommandResult {
    let mut realm = String::new();
    let mut character = String::new();
    for o in options {
        match &o.name[..] {
            "realm" => {
                if let Some(CommandDataOptionValue::String(r)) = o.resolved.as_ref() {
                    realm = r
                        .trim()
                        .to_ascii_lowercase()
                        .replace(' ', "-")
                        .replace('\'', "");
                }
            }
            "character" => {
                if let Some(CommandDataOptionValue::String(c)) = o.resolved.as_ref() {
                    character = c.trim().to_ascii_lowercase();
                }
            }
            _ => {}
        }
    }

    let access_token = get_access_token(ctx).await?;

    let date_format = "%a, %b %-d %Y at %-I:%M%P";

    // Get character last login time (and check if they exist)
    let last_login: String = match get_character(&realm, &character, &access_token).await {
        Ok(c) => format!(
            "Player last seen on {}",
            match c.last_login_local() {
                None => String::from("UNKNOWN"),
                Some(l) => l.format(date_format).to_string(),
            }
        ),
        Err(e)
            if e.status() == Some(StatusCode::NOT_FOUND)
                || e.status() == Some(StatusCode::FORBIDDEN) =>
        {
            interaction
                .edit_original_interaction_response(&ctx.http, |response| {
                    response.content(format!("Unable to find {character} on {realm}"))
                })
                .await?;
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };

    // Get JSON info of character's appearance
    let media: CharacterMedia =
        match get_character_media(&realm, &character, &access_token, None, None).await {
            Ok(m) => m,
            Err(e) if e.status() == Some(StatusCode::NOT_FOUND) => {
                interaction
                    .edit_original_interaction_response(&ctx.http, |response| {
                        response.content(format!(
                            "Unable to find images for {} on {}",
                            character, realm
                        ))
                    })
                    .await?;
                return Ok(());
            }
            Err(e) => return Err(e.into()),
        };
    let last_modified: Option<String> = media
        .last_modified
        .map(|lm| format!("Image updated on {}", lm.format(date_format)));

    let msg_content = if let Some(last_modified) = last_modified {
        format!("{last_modified}\n{last_login}")
    } else {
        last_login
    };

    let mut found_raw = false; //if we found "row" png image with transparency
    let mut image_url: Option<String> = None; //URL of image we'll use

    // First check assets array for main-raw (PNG) image, then main image
    if let Some(assets) = media.assets {
        if let Some(raw) = assets.iter().find(|&a| a.key == "main-raw") {
            found_raw = true;
            image_url = Some(raw.value.clone());
        } else if let Some(main) = assets.iter().find(|&a| a.key == "main") {
            image_url = Some(main.value.clone());
        }
    }
    // If image wasn't in assets (or it didn't exist) check other render_url field
    if image_url.is_none() {
        image_url = media.render_url;
    }
    let image_url = if let Some(url) = image_url {
        url
    } else {
        return Err("Unable to find character imagery".into());
    };
    // If we didn't find a transparent-background PNG image, just send the URL for whatever image we do have (discord will convert it)
    if !found_raw {
        interaction
            .edit_original_interaction_response(&ctx.http, |response| {
                response.content(format!("{msg_content}\n{image_url}"))
            })
            .await?;
        return Ok(());
    }

    // Otherwise, fetch, decode, crop, and attach PNG image

    // Fetch and decode image, assume it's a PNG
    let image_bytes = reqwest::get(&image_url).await?.bytes().await?;
    let mut image =
        image::load_from_memory_with_format(&image_bytes, ImageFormat::Png)?.into_rgba8();
    let (width, height) = image.dimensions();

    // Find boundaries of image content
    let mut top = 0;
    let mut bottom = height;
    let mut left = 0;
    let mut right = width;

    // find top most non-blank pixel
    (0..height).any(|y| {
        (0..width).any(|x| {
            if image[(x, y)][3] > 0 {
                // check alpha channel for non-transparency
                top = y;
                return true;
            }
            false
        })
    });

    // find bottom most non-blank pixel
    (top..height).rev().any(|y| {
        (0..width).any(|x| {
            if image[(x, y)][3] > 0 {
                bottom = (y + 1).min(height);
                return true;
            }
            false
        })
    });
    // find left most non-blank pixel
    (0..width).any(|x| {
        (top..bottom).any(|y| {
            if image[(x, y)][3] > 0 {
                left = x;
                return true;
            }
            false
        })
    });
    // find right most non-blank pixel
    (left..width).rev().any(|x| {
        (top..bottom).any(|y| {
            if image[(x, y)][3] > 0 {
                right = (x + 1).min(width);
                return true;
            }
            false
        })
    });

    // Crop image and encode PNG into buffer
    let cropped_image =
        imageops::crop(&mut image, left, top, right - left, bottom - top).to_image();
    let mut cropped_buffer = Vec::new();
    PngEncoder::new(&mut cropped_buffer).write_image(
        &cropped_image,
        right - left,
        bottom - top,
        ColorType::Rgba8,
    )?;

    // Send message with attached cropped image
    interaction
        .edit_original_interaction_response(&ctx.http, |response| response.content(msg_content))
        .await?;
    interaction
        .channel_id
        .send_message(&ctx.http, |m| {
            m.add_file(AttachmentType::Bytes {
                data: Cow::from(cropped_buffer),
                filename: format!("{realm}-{character}.png"),
            })
        })
        .await?;

    Ok(())
}

// Replies with an embed containing WoW character info
pub async fn character(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    options: &Vec<CommandDataOption>,
) -> CommandResult {
    let mut realm_name = String::new();
    let mut character_name = String::new();
    for o in options {
        match &o.name[..] {
            "realm" => {
                if let Some(CommandDataOptionValue::String(r)) = o.resolved.as_ref() {
                    realm_name = r
                        .trim()
                        .to_ascii_lowercase()
                        .replace(' ', "-")
                        .replace('\'', "");
                }
            }
            "character" => {
                if let Some(CommandDataOptionValue::String(c)) = o.resolved.as_ref() {
                    character_name = c.trim().to_ascii_lowercase();
                }
            }
            _ => {}
        }
    }

    let access_token = get_access_token(ctx).await?;

    let character: Character =
        match get_character(&realm_name, &character_name, &access_token).await {
            Ok(c) => c,
            Err(e)
                if e.status() == Some(StatusCode::NOT_FOUND)
                    || e.status() == Some(StatusCode::FORBIDDEN) =>
            {
                interaction
                    .edit_original_interaction_response(&ctx.http, |response| {
                        response.content(format!(
                            "Unable to find {} on {}",
                            character_name, realm_name
                        ))
                    })
                    .await?;
                return Ok(());
            }
            Err(e) => return Err(e.into()),
        };

    let inset_url: Option<String> = match get_character_media(
        &realm_name,
        &character_name,
        &access_token,
        Some(character.race.id),
        Some(&character.gender.t),
    )
    .await
    {
        Ok(media) => media.assets.and_then(|assets| {
            assets.iter().find_map(|a| {
                if a.key == "inset" {
                    let seconds =
                        if let Ok(t) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
                            t.as_secs()
                        } else {
                            warn!(time = ?SystemTime::now(), "SystemTime before UNIX_EPOCH");
                            0
                        };
                    Some(a.value.clone() + &format!("?{seconds}"))
                } else {
                    None
                }
            })
        }),
        Err(e) => {
            error!(%e, "Error getting character media");
            None
        }
    };

    let stats: CharacterStats =
        get_character_statistics(&realm_name, &character_name, &access_token).await?;
    let titles: CharacterTitles =
        get_character_titles(&realm_name, &character_name, &access_token).await?;

    let titled_name = titles
        .active_title
        .map_or(character.name.clone(), |active| {
            active.display_string.replace("{name}", &character.name)
        });

    let guild_name = if let Some(guild) = &character.guild {
        format!("\n\u{276e}{}\u{276f}", guild.name)
    } else {
        String::new()
    };

    let covenant_info = if let Some(covenant) = &character.covenant_progress {
        format!(
            "\n{} Rank {}",
            covenant.chosen_covenant.name, covenant.renown_level
        )
    } else {
        String::new()
    };

    let active_spec = if let Some(spec) = &character.active_spec {
        format!(" {spec}")
    } else {
        String::new()
    };

    interaction
        .edit_original_interaction_response(&ctx.http, |response| {
            response.embed(|e| {
                e.title(format!("{titled_name}{guild_name}"))
                    .description(format!(
                        "Level {} {}{} {}{}",
                        character.level,
                        character.race,
                        active_spec,
                        character.character_class,
                        covenant_info
                    ))
                    .url(format!(
                        "https://worldofwarcraft.com/en-us/character/us/{}/{}/",
                        realm_name, character_name
                    ))
                    .colour(CLASS_COLOURS[(character.character_class.id - 1) as usize])
                    .field(
                        "ILVL",
                        format!(
                            "{}/{}",
                            character.equipped_item_level, character.average_item_level
                        ),
                        true,
                    )
                    .field("Health", stats.health, true)
                    .field(&stats.power_type, stats.power, true)
                    .field("\u{200B}", "\u{200B}", false)
                    .field("Strength", &stats.strength, true)
                    .field("Agility", &stats.agility, true)
                    .field("Intellect", &stats.intellect, true)
                    .field("Stamina", &stats.stamina, true)
                    .field("Armor", &stats.armor, true)
                    .field("\u{200B}", "\u{200B}", false)
                    .field("Crit", format!("{:.2}%", &stats.crit()), true)
                    .field("Haste", format!("{:.2}%", &stats.haste()), true)
                    .field("Mastery", &stats.mastery, true)
                    .field(
                        "Versatility",
                        format!("{:.2}%", &stats.versatility_damage_done_bonus),
                        true,
                    );
                if let Some(inset_url) = inset_url {
                    e.image(inset_url);
                }
                if let Some(last_login) = character.last_login_utc() {
                    e.timestamp(last_login.to_rfc3339());
                }
                e
            })
        })
        .await?;

    Ok(())
}

// Replies with a list of matching character names and their realms
pub async fn search(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    options: &[CommandDataOption],
) -> CommandResult {
    let character = if let Some(o) = options.get(0) {
        if let Some(CommandDataOptionValue::String(c)) = o.resolved.as_ref() {
            c.trim().to_ascii_lowercase()
        } else {
            return Err("Invalid character name argument".into());
        }
    } else {
        return Err("Missing required character name argument".into());
    };

    let builder = reqwest::Client::builder()
        .gzip(true)
        .brotli(true)
        .build()?
        .request(
            reqwest::Method::GET,
            format!(
                "https://worldofwarcraft.com/en-us/search/character?q={}",
                character
            ),
        )
        .header("Host", "worldofwarcraft.com")
        .header(
            "User-Agent",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:83.0) Gecko/20100101 Firefox/83.0",
        )
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
        )
        .header("Accept-Language", "en-US,en;q=0.5")
        .header("Accept-Encoding", "gzip, br")
        .header("Referer", "https://worldofwarcraft.com/en-us/")
        .header("Connection", "keep-alive")
        .header("Upgrade-Insecure-Requests", "1")
        .header("Pragma", "no-cache")
        .header("Cache-Control", "no-cache");
    #[allow(clippy::unwrap_used)]
    let char_regex = regex::Regex::new(r#"href="/en-us/character/us/([\w-]+)/(\w+)""#).unwrap();

    let html = match builder.send().await {
        Ok(r) => match r.text().await {
            Ok(t) => t,
            Err(e) => {
                return Err(e.into());
            }
        },
        Err(e) => {
            return Err(e.into());
        }
    };

    let mut content = vec![];

    for caps in char_regex.captures_iter(&html) {
        #[allow(clippy::unwrap_used)]
        content.push(format!(
            "{}-{}",
            caps.get(2).unwrap().as_str(),
            caps.get(1).unwrap().as_str()
        ));
    }

    interaction
        .edit_original_interaction_response(&ctx.http, |response| {
            response.content(content.join("\n"))
        })
        .await?;

    Ok(())
}

pub async fn realm(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    options: &[CommandDataOption],
) -> CommandResult {
    let arg = if let Some(o) = options.get(0) {
        if let Some(CommandDataOptionValue::String(r)) = o.resolved.as_ref() {
            r
        } else {
            return Err("Invalid realm name argument".into());
        }
    } else {
        return Err("Missing required realm name argument".into());
    };
    let realm_slug = arg
        .trim()
        .to_ascii_lowercase()
        .replace(' ', "-")
        .replace('\'', "");

    let access_token = get_access_token(ctx).await?;

    let client = reqwest::Client::new();

    let search: Search = client.get(&format!("https://us.api.blizzard.com/data/wow/search/connected-realm?namespace=dynamic-us&locale=en_US&realms.slug={realm_slug}&orderby=id&_page=1&access_token={access_token}"))
        .send().await?.json().await?;

    if search.results.is_empty() || search.results[0].data.realms.is_empty() {
        interaction
            .edit_original_interaction_response(&ctx.http, |response| {
                response.content(format!("Unable to find {arg}"))
            })
            .await?;
        return Ok(());
    }

    let realm = if let Some(r) = search.results[0]
        .data
        .realms
        .iter()
        .find(|&r| r.slug == realm_slug)
    {
        r
    } else {
        interaction
            .edit_original_interaction_response(&ctx.http, |response| {
                response.content(format!("Unable to find {arg}"))
            })
            .await?;
        return Ok(());
    };

    let realm_name = realm.name.en_us.as_ref().unwrap_or(arg);
    let realm_data = &search.results[0].data;

    let content = if realm_data.status.t == "UP" {
        if realm_data.has_queue {
            format!("{realm_name} is online but has a queue")
        } else {
            format!("{realm_name} is online and has no queue")
        }
    } else {
        format!("{realm_name} is offline")
    };

    interaction
        .edit_original_interaction_response(&ctx.http, |response| response.content(content))
        .await?;

    Ok(())
}
