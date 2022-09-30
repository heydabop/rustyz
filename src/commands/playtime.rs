use crate::error::{CommandError, CommandResult};
use crate::model::DB;
use crate::util;
use chrono::{prelude::*, Duration};
use regex::{Match, Regex};
use serenity::client::Context;
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOption, CommandDataOptionValue,
};
use serenity::model::id::GuildId;
use sqlx::Row;
use std::collections::HashMap;
use std::str::FromStr;

struct GameDate {
    date: DateTime<FixedOffset>,
    game: String,
}

struct GameTime {
    time: Duration,
    game: String,
}

// Replies to msg with the cumulative playtime of all users in the guild
// Takes a single optional argument of a username to filter playtime for
pub async fn playtime(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let guild_id = match interaction.guild_id {
        Some(g) => g,
        None => {
            interaction
                .edit_original_interaction_response(&ctx.http, |response| {
                    response.content("Command can only be used in a server")
                })
                .await?;
            return Ok(());
        }
    };

    let (user_ids, username): (Vec<i64>, Option<String>) = match user_ids_and_name_from_option(
        ctx,
        guild_id,
        interaction.data.options.get(0),
    )
    .await?
    {
        Some(u) => (u.0, u.1),
        None => return Ok(()),
    };

    send_message_with_buttons(ctx, interaction, &user_ids, &username, None).await?;

    Ok(())
}

// Replies to msg with the cumulative playtime since the given time period of all users in the guild
// Takes two arguments
// First (required): human readable time duration (2 days, 1 hour, 3 months, etc)
// Second (optional): username to filter playtime for
pub async fn recent_playtime(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
) -> CommandResult {
    let guild_id = match interaction.guild_id {
        Some(g) => g,
        None => {
            interaction
                .edit_original_interaction_response(&ctx.http, |response| {
                    response.content("Command can only be used in a server")
                })
                .await?;
            return Ok(());
        }
    };

    let arg = if let CommandDataOptionValue::String(c) =
        match interaction.data.options[0].resolved.as_ref() {
            Some(r) => r,
            None => return Err("Missing required arguments".into()),
        } {
        String::from(c.trim())
    } else {
        String::new()
    };
    let duration_regex = Regex::new(
        r#"(?i)(?:(?:(?:(\d+)\s+years?)|(?:(\d+)\s+months?)|(?:(\d+)\s+weeks?)|(?:(\d+)\s+days?)|(?:(\d+)\s+hours?)|(?:(\d+)\s+minutes?)|(?:(\d+)\s+seconds?))\s?)+"#,
    )?;
    let now = Local::now();
    let now = now.with_timezone(now.offset());
    let start_date: DateTime<FixedOffset> = if let Some(captures) = duration_regex.captures(&arg) {
        let years = get_digit_from_match(captures.get(1))?;
        let months = get_digit_from_match(captures.get(2))?;
        let weeks = get_digit_from_match(captures.get(3))?;
        let days = get_digit_from_match(captures.get(4))?;
        let hours = get_digit_from_match(captures.get(5))?;
        let minutes = get_digit_from_match(captures.get(6))?;
        let seconds = get_digit_from_match(captures.get(7))?;
        let month_days = match months_to_days(now, months) {
            Some(d) => d,
            None => return Err("date overflow".into()),
        };
        now - Duration::days(years * 365)
            - Duration::days(month_days)
            - Duration::days(weeks * 7)
            - Duration::days(days)
            - Duration::hours(hours)
            - Duration::minutes(minutes)
            - Duration::seconds(seconds)
    } else {
        interaction
            .edit_original_interaction_response(&ctx.http, |response| {
                response.content("```Unable to parse time```")
            })
            .await?;
        return Ok(());
    };
    let (user_ids, username) = match user_ids_and_name_from_option(
        ctx,
        guild_id,
        interaction.data.options.get(1),
    )
    .await?
    {
        Some(u) => (u.0, u.1),
        None => return Ok(()),
    };

    send_message_with_buttons(ctx, interaction, &user_ids, &username, Some(start_date)).await?;

    Ok(())
}

fn get_digit_from_match(mat: Option<Match>) -> Result<i64, std::num::ParseIntError> {
    match mat {
        None => Ok(0),
        Some(mat) => i64::from_str(mat.as_str()),
    }
}

// takes months and turns it to days by counting the days of each month, supports + or - months
fn months_to_days(now: DateTime<FixedOffset>, mut months: i64) -> Option<i64> {
    let mut end = now;
    loop {
        if months == 0 {
            break Some((end - now).num_days());
        }
        end = match end.checked_add_signed(Duration::days(
            NaiveDate::from_ymd(
                match end.month() {
                    12 => end.year() + 1,
                    _ => end.year(),
                },
                match end.month() {
                    12 => 1,
                    _ => end.month() + 1,
                },
                1,
            )
            .signed_duration_since(NaiveDate::from_ymd(end.year(), end.month(), 1))
            .num_days(),
        )) {
            Some(e) => e,
            None => return None,
        };
        if months > 0 {
            months -= 1;
        } else {
            months += 1;
        }
    }
}

async fn user_ids_and_name_from_option(
    ctx: &Context,
    guild_id: GuildId,
    option: Option<&CommandDataOption>,
) -> Result<Option<(Vec<i64>, Option<String>)>, CommandError> {
    let mut username: Option<String> = None;
    let user_ids: Vec<i64> = if option.is_none() {
        // get list of user IDs in channel
        let members = util::collect_members_guild_id(ctx, guild_id).await?;
        members
            .iter()
            .map(|m| i64::try_from(*m.0.as_u64()))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let user_id = match option {
            Some(o) => {
                if let Some(CommandDataOptionValue::User(u, _)) = o.resolved.as_ref() {
                    u.id
                } else {
                    return Ok(None);
                }
            }
            None => return Ok(None),
        };
        if let Some(guild) = ctx.cache.guild(guild_id) {
            if let Ok(member) = guild.member(ctx, user_id).await {
                username = member.nick;
            }
        }
        if username.is_none() {
            let members = util::collect_members_guild_id(ctx, guild_id).await?;
            username = if let Some(member) = members.get(&user_id) {
                match &member.nick {
                    Some(nick) => Some(nick.clone()),
                    None => Some(member.user.name.clone()),
                }
            } else {
                return Ok(None);
            };
        }
        vec![i64::try_from(user_id.0)?]
    };

    Ok(Some((user_ids, username)))
}

pub async fn gen_playtime_message(
    ctx: &Context,
    user_ids: &[i64],
    username: &Option<String>,
    start_date: Option<DateTime<FixedOffset>>,
    end_date: DateTime<FixedOffset>,
    offset: usize,
) -> Result<String, CommandError> {
    // get all rows with a user id in the channel
    let rows = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        let db = data.get::<DB>().unwrap();
        sqlx::query(r#"SELECT create_date, user_id, game_name FROM user_presence WHERE user_id = any($1) AND (create_date > $2) IS NOT FALSE AND create_date <= $3 ORDER BY create_date"#).bind(user_ids).bind(start_date).bind(end_date).fetch_all(db).await?
    };
    if rows.is_empty() {
        return Ok(format!(
            "```No recorded playtime{}```",
            if let Some(username) = username {
                format!(" for {}", username)
            } else {
                String::new()
            }
        ));
    }

    let mut gametimes: HashMap<String, Duration> = HashMap::new(); // stores how long each game has been played
    let mut last_user_game: HashMap<i64, GameDate> = HashMap::new(); // tracks the last game a user was "seen" playing as we iterate through the rows
    let first_time = rows[0].get::<DateTime<FixedOffset>, _>(0); // used to display in message how long players have been tracked
    for row in &rows {
        let date = row.get::<DateTime<FixedOffset>, _>(0);
        let user_id = row.get::<i64, _>(1);
        let game = row.get::<Option<String>, _>(2);

        let last = match last_user_game.get(&user_id) {
            Some(l) => l,
            None => {
                // user wasn't playing anything, record new entry if user is playing something now, otherwise just continue
                if let Some(game) = game {
                    last_user_game.insert(user_id, GameDate { date, game });
                }
                continue;
            }
        };

        // user is still playing the same thing
        if let Some(game) = &game {
            if game == &last.game {
                continue;
            }
        }

        // user is playing something different (or nothing), record how long they played last game
        if let Some(gametime) = gametimes.get_mut(&last.game) {
            // increment existing game time
            *gametime = *gametime + (date - last.date);
        } else {
            // or insert new entry for first-seen game
            gametimes.insert(last.game.clone(), date - last.date);
        }

        // record what is now playing, if anything
        match game {
            Some(game) => last_user_game.insert(user_id, GameDate { date, game }),
            None => last_user_game.remove(&user_id),
        };
    }

    // users are currently playing game at the time of this command so we have no row for them stopping
    for last in last_user_game.values() {
        if let Some(gametime) = gametimes.get_mut(&last.game) {
            // increment existing game time
            *gametime = *gametime + (end_date - last.date);
        } else {
            // or insert new entry for first-seen game
            gametimes.insert(last.game.clone(), end_date - last.date);
        }
    }

    // convert HashMap to Vec so we can sort it by time in descending order
    let mut total_time = Duration::zero();
    let mut gametimes: Vec<GameTime> = match gametimes
        .iter()
        .map(|(game, time)| {
            total_time = match total_time.checked_add(time) {
                Some(t) => t,
                None => return None,
            };
            Some(GameTime {
                time: *time,
                game: game.clone(),
            })
        })
        .collect()
    {
        Some(g) => g,
        None => return Err("time overflow".into()),
    };

    if gametimes.is_empty() {
        return Ok(format!(
            "```No recorded playtime{}```",
            if let Some(username) = username {
                format!(" for {}", username)
            } else {
                String::new()
            }
        ));
    }

    gametimes.push(GameTime {
        time: total_time,
        game: String::from("All Games"),
    });
    gametimes.sort_by(|a, b| b.time.cmp(&a.time));
    let min_offset = offset.max(0);
    let max_offset = (offset + 15).min(gametimes.len());
    let gametimes = &gametimes[min_offset..max_offset];
    let longest_game_name = gametimes.iter().map(|g| g.game.len()).max().unwrap_or(0); // get longest game name so we can pad shorter game names and lineup times

    let mut lines = Vec::with_capacity(gametimes.len());
    #[allow(clippy::cast_precision_loss)]
    for gametime in gametimes {
        lines.push(format!(
            "{:>width$} \u{2014} {:.2}\n",
            gametime.game,
            (gametime.time.num_seconds()) as f64 / 3600_f64,
            width = longest_game_name
        ));
    }

    let mut time_format_string = "%b %d, %Y";
    if let Some(start_date) = start_date {
        if (end_date - start_date).num_days() < 1 {
            time_format_string = "%l:%M%p";
        }
    };

    Ok(format!(
        "```{} {}\n\n{}```",
        if let Some(username) = username {
            format!("{} since", username)
        } else {
            String::from("Since")
        },
        first_time
            .with_timezone(end_date.offset())
            .format(time_format_string),
        lines.concat()
    ))
}

async fn send_message_with_buttons(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    user_ids: &[i64],
    username: &Option<String>,
    start_date: Option<DateTime<FixedOffset>>,
) -> CommandResult {
    let now = Local::now();
    let now = now.with_timezone(now.offset());
    let content = gen_playtime_message(ctx, user_ids, username, start_date, now, 0).await?;

    interaction
        .edit_original_interaction_response(&ctx.http, |response| response.content(&content))
        .await?;

    Ok(())
}
