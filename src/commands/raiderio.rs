use reqwest::{StatusCode, Url};
use serde::Deserialize;
use serenity::client::Context;
use serenity::framework::standard::{CommandError, CommandResult};
use serenity::model::application::interaction::{
    application_command::{ApplicationCommandInteraction, CommandDataOptionValue},
    InteractionResponseType,
};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::time::SystemTime;

const PLUSSES: [&str; 4] = ["", "+", "++", "+++"];

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
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
    mythic_plus_recent_runs: Vec<MythicPlusRun>,
    raid_progression: RaidProgression,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct MythicPlusSeasonScores {
    season: String,
    scores: MythicPlusScores,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct MythicPlusScores {
    all: f32,
    dps: f32,
    healer: f32,
    tank: f32,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
struct RaidProgression {
    #[serde(rename = "castle-nathria")]
    castle_nathria: Raid,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct Raid {
    summary: String,
    total_bosses: u8,
    normal_bosses_killed: u8,
    heroic_bosses_killed: u8,
    mythic_bosses_killed: u8,
}

#[derive(Debug, Deserialize)]
struct StaticData {
    dungeons: Vec<Dungeon>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct Dungeon {
    id: u32,
    short_name: String,
}

// Takes in the arg `<character>-<realm>` and replies with stats from raider.io
pub async fn raiderio(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    interaction
        .create_interaction_response(&ctx.http, |response| {
            response.kind(InteractionResponseType::DeferredChannelMessageWithSource)
        })
        .await?;

    let mut character = if let CommandDataOptionValue::String(c) =
        interaction.data.options[0].resolved.as_ref().unwrap()
    {
        String::from(c.trim())
    } else {
        String::new()
    };
    let mut realm = if let CommandDataOptionValue::String(r) =
        interaction.data.options[1].resolved.as_ref().unwrap()
    {
        r.trim().replace(' ', "-").replace('\'', "")
    } else {
        String::new()
    };
    character.make_ascii_lowercase();
    realm.make_ascii_lowercase();

    let client = reqwest::Client::new();
    let dungeons = client
        .get(Url::parse("https://raider.io/api/v1/mythic-plus/static-data?expansion_id=8").unwrap())
        .send()
        .await?
        .json::<StaticData>()
        .await?
        .dungeons;

    let profile = match client.get(Url::parse(&format!("https://raider.io/api/v1/characters/profile?region=us&realm={}&name={}&fields=raid_progression%2Cmythic_plus_scores_by_season%3Acurrent%2Cmythic_plus_best_runs%3Aall%2Cmythic_plus_highest_level_runs%2Cmythic_plus_recent_runs", realm, character)).unwrap()).send().await?.error_for_status() {
        Ok(resp) => if let Ok(profile) = resp.json::<CharacterProfile>().await {
            profile
        } else {
            // assume raider.io is giving us a 400 response as a json error under a 200 http response
            interaction
                .edit_original_interaction_response(&ctx.http, |response| {
                    response.content(format!("Unable to find raiderio profile for {} on {}", character, realm))
                })
                .await?;
            return Ok(());
        }
        Err(e) => {
            if e.status() == Some(StatusCode::NOT_FOUND) || e.status() == Some(StatusCode::BAD_REQUEST) {
                interaction
                    .edit_original_interaction_response(&ctx.http, |response| {
                        response.content(format!("Unable to find raiderio profile for {} on {}", character, realm))
                    })
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

    let highest_runs = format_runs(&profile.mythic_plus_highest_level_runs, 5);
    let recent_runs = format_runs(&profile.mythic_plus_recent_runs, 5);

    let best_runs = if profile.mythic_plus_best_runs.is_empty() {
        String::from("No runs")
    } else {
        let mut best_runs_by_dungeon: HashMap<String, Option<&MythicPlusRun>> = HashMap::new();
        let mut num_dungeons = 0;
        let mut longest_name: usize = 0;
        for run in &profile.mythic_plus_best_runs {
            if num_dungeons == dungeons.len() {
                break;
            }
            if let Some(run) = best_runs_by_dungeon.get(&run.short_name) {
                if run.is_some() {
                    continue;
                }
            }
            best_runs_by_dungeon.insert(run.short_name.clone(), Some(run));
            num_dungeons += 1;
            longest_name = run.short_name.len().max(longest_name);
        }

        let mut sorted_best_runs = Vec::with_capacity(num_dungeons);
        for (short_name, run) in best_runs_by_dungeon {
            if let Some(run) = run {
                sorted_best_runs.push(format!(
                    "`{:width$}` {}{}",
                    short_name,
                    run.mythic_level,
                    PLUSSES[run.num_keystone_upgrades as usize],
                    width = longest_name
                ));
            } else {
                sorted_best_runs.push(format!("`{:width$}` --", short_name, width = longest_name));
            }
        }
        sorted_best_runs.sort();

        sorted_best_runs.join("\n")
    };

    interaction
        .edit_original_interaction_response(&ctx.http, |response| {
            response.embed(|e| {
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
                    .field("Recent Runs", recent_runs, true)
                    .field("Best Runs by Dungeon", best_runs, true)
            })
        })
        .await?;

    Ok(())
}

fn format_runs(runs: &[MythicPlusRun], count: usize) -> String {
    if runs.is_empty() {
        return String::from("No runs");
    }
    let mut s = String::with_capacity(5 * count);
    for run in runs.iter().take(count) {
        let _ = writeln!(
            s,
            "{} {}{}",
            run.short_name, run.mythic_level, PLUSSES[run.num_keystone_upgrades as usize]
        );
    }
    s.pop();
    s
}
