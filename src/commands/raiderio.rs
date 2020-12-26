use reqwest::{StatusCode, Url};
use serde::Deserialize;
use serenity::client::Context;
use serenity::framework::standard::{macros::command, Args, CommandError, CommandResult};
use serenity::model::channel::Message;
use std::time::SystemTime;

const PLUSSES: [&str; 4] = ["", "+", "++", "+++"];

#[allow(dead_code)]
#[derive(Deserialize)]
struct CharacterProfile {
    name: String,
    race: String,
    class: String,
    active_spec_name: Option<String>,
    last_crawled_at: String,
    profile_url: String,
    thumbnail_url: String,
    mythic_plus_scores_by_season: Vec<MythicPlusSeasonScores>,
    mythic_plus_best_runs: Vec<MythicPlusRun>,
    mythic_plus_highest_level_runs: Vec<MythicPlusRun>,
    raid_progression: RaidProgression,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct MythicPlusSeasonScores {
    season: String,
    scores: MythicPlusScores,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct MythicPlusScores {
    all: f32,
    dps: f32,
    healer: f32,
    tank: f32,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct MythicPlusRun {
    dungeon: String,
    short_name: String,
    mythic_level: u8,
    completed_at: String,
    clear_time_ms: u32,
    num_keystone_upgrades: u8,
    score: f32,
    url: String,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct RaidProgression {
    #[serde(rename = "castle-nathria")]
    castle_nathria: Raid,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct Raid {
    summary: String,
    total_bosses: u8,
    normal_bosses_killed: u8,
    heroic_bosses_killed: u8,
    mythic_bosses_killed: u8,
}

// Takes in the arg `<character>-<realm>` and replies with stats from raider.io
#[command]
pub async fn raiderio(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    // Parse out character and realm names from single string arg `<character>-<realm>`
    let mut arg = args.rest().to_string();
    arg.make_ascii_lowercase();
    let char_realm: Vec<&str> = arg.splitn(2, '-').collect();
    if char_realm.len() != 2 {
        msg.channel_id
            .say(&ctx.http, "`Usage: !raiderio name-realm`")
            .await?;
        return Ok(());
    }
    let character = char_realm[0].trim();
    let realm = char_realm[1].trim().replace(" ", "-").replace("'", "");

    let client = reqwest::Client::new();
    let profile = match client.get(Url::parse(&format!("https://raider.io/api/v1/characters/profile?region=us&realm={}&name={}&fields=raid_progression%2Cmythic_plus_scores_by_season%3Acurrent%2Cmythic_plus_best_runs%2Cmythic_plus_highest_level_runs", realm, character)).unwrap()).send().await?.error_for_status() {
        Ok(resp) => resp.json::<CharacterProfile>().await?,
        Err(e) => {
            if e.status() == Some(StatusCode::NOT_FOUND) {
                msg.channel_id
                    .say(
                        &ctx.http,
                        format!("Unable to find {} on {}", character, realm),
                    )
                    .await?;
                return Ok(());
            }
            return Err(CommandError::from(e));
        }
    };

    let thumbnail_url = if let Ok(t) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        format!("{}?{}", profile.thumbnail_url, t.as_secs())
    } else {
        profile.thumbnail_url.clone()
    };

    let mut highest_runs = String::new();
    for i in 0..5.min(profile.mythic_plus_highest_level_runs.len()) {
        let run = &profile.mythic_plus_highest_level_runs[i];
        highest_runs.push_str(&format!(
            "{} {}{}\n",
            run.short_name, run.mythic_level, PLUSSES[run.num_keystone_upgrades as usize]
        ));
    }

    msg.channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title(format!("{}-{}", profile.name, realm))
                    .timestamp(profile.last_crawled_at)
                    .url(profile.profile_url)
                    .thumbnail(thumbnail_url)
                    .field(
                        "Mythic+ Score",
                        profile.mythic_plus_scores_by_season[0].scores.all,
                        true,
                    )
                    .field("Highest Runs", highest_runs, true)
            });
            m
        })
        .await?;

    Ok(())
}
