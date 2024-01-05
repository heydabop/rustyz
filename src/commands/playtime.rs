use crate::error::{CommandError, CommandResult};
use crate::model::DB;
use crate::util;
use chrono::{prelude::*, Duration};
use regex::{Match, Regex};
use serenity::all::{ButtonStyle, CommandDataOption, CommandDataOptionValue, CommandInteraction};
use serenity::builder::CreateActionRow;
use serenity::builder::{CreateButton, EditInteractionResponse};
use serenity::client::Context;
use serenity::model::id::GuildId;
use std::collections::HashMap;
use std::str::FromStr;

struct GameDate {
    date: DateTime<Utc>,
    game: String,
}

struct GameTime {
    time: Duration,
    game: String,
}

pub const OFFSET_INC: u16 = 15;

// Replies to msg with the cumulative playtime of all users in the guild
// Takes a single optional argument of a username to filter playtime for
pub async fn playtime(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let Some(guild_id) = interaction.guild_id else {
        interaction
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new().content("Command can only be used in a server"),
            )
            .await?;
        return Ok(());
    };

    let (user_ids, username): (Vec<i64>, Option<String>) =
        match user_ids_and_name_from_option(ctx, guild_id, interaction.data.options.first()).await?
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
pub async fn recent_playtime(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let Some(guild_id) = interaction.guild_id else {
        interaction
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new().content("Command can only be used in a server"),
            )
            .await?;
        return Ok(());
    };

    let arg = if let CommandDataOptionValue::String(c) = &interaction.data.options[0].value {
        String::from(c.trim())
    } else {
        return Err("Missing required arguments".into());
    };
    let duration_regex = Regex::new(
        r"(?i)(?:(?:(?:(\d+)\s+years?)|(?:(\d+)\s+months?)|(?:(\d+)\s+weeks?)|(?:(\d+)\s+days?)|(?:(\d+)\s+hours?)|(?:(\d+)\s+minutes?)|(?:(\d+)\s+seconds?))\s?)+",
    )?;
    let now = Utc::now();
    let start_date: DateTime<Utc> = if let Some(captures) = duration_regex.captures(&arg) {
        let years = get_digit_from_match(captures.get(1))?;
        let months = get_digit_from_match(captures.get(2))?;
        let weeks = get_digit_from_match(captures.get(3))?;
        let days = get_digit_from_match(captures.get(4))?;
        let hours = get_digit_from_match(captures.get(5))?;
        let minutes = get_digit_from_match(captures.get(6))?;
        let seconds = get_digit_from_match(captures.get(7))?;
        let Some(month_days) = months_to_days(now, months) else {
            return Err("date overflow".into());
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
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new().content("```Unable to parse time```"),
            )
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
fn months_to_days(now: DateTime<Utc>, mut months: i64) -> Option<i64> {
    let mut end = now;
    loop {
        if months == 0 {
            break Some((end - now).num_days());
        }
        end = match end.checked_add_signed(Duration::days(
            NaiveDate::from_ymd_opt(
                match end.month() {
                    12 => end.year() + 1,
                    _ => end.year(),
                },
                match end.month() {
                    12 => 1,
                    _ => end.month() + 1,
                },
                1,
            )?
            .signed_duration_since(NaiveDate::from_ymd_opt(end.year(), end.month(), 1)?)
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
            .map(|m| i64::try_from(m.0.get()))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let user_id = match option {
            Some(o) => {
                if let CommandDataOptionValue::User(u) = o.value {
                    u
                } else {
                    return Ok(None);
                }
            }
            None => return Ok(None),
        };
        let guild = { ctx.cache.guild(guild_id).map(|g| g.clone()) };
        if let Some(guild) = guild {
            if let Ok(member) = guild.member(ctx, user_id).await {
                username = member.nick.clone();
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
        vec![i64::from(user_id)]
    };

    Ok(Some((user_ids, username)))
}

pub async fn gen_playtime_message(
    ctx: &Context,
    user_ids: &[i64],
    username: &Option<String>,
    start_date: Option<DateTime<Utc>>,
    end_date: DateTime<Utc>,
    offset: usize,
) -> Result<String, CommandError> {
    // get all rows with a user id in the channel
    let rows = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        let db = data.get::<DB>().unwrap();
        #[allow(clippy::panic)]
        sqlx::query!(r#"SELECT create_date, user_id, game_name FROM user_presence WHERE user_id = any($1) AND (create_date > $2) IS NOT FALSE AND create_date <= $3 ORDER BY create_date"#, user_ids, start_date, end_date).fetch_all(db).await?
    };
    if rows.is_empty() {
        return Ok(format!(
            "```No recorded playtime{}```",
            if let Some(username) = username {
                format!(" for {username}")
            } else {
                String::new()
            }
        ));
    }

    let mut gametimes: HashMap<String, Duration> = HashMap::new(); // stores how long each game has been played
    let mut last_user_game: HashMap<i64, GameDate> = HashMap::new(); // tracks the last game a user was "seen" playing as we iterate through the rows
    let first_time: DateTime<Utc> = rows[0].create_date; // used to display in message how long players have been tracked
    for row in rows {
        let date: DateTime<Utc> = row.create_date;
        let user_id: i64 = row.user_id;
        let game: Option<String> = row.game_name;

        let Some(last) = last_user_game.get(&user_id) else {
            // user wasn't playing anything, record new entry if user is playing something now, otherwise just continue
            if let Some(game) = game {
                last_user_game.insert(user_id, GameDate { date, game });
            }
            continue;
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
                format!(" for {username}")
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
    let max_offset = (offset + usize::from(OFFSET_INC)).min(gametimes.len());
    let total_lines = gametimes.len();
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
        "```{} {} - Page {}/{}\n\n{}```",
        if let Some(username) = username {
            format!("{username} since")
        } else {
            String::from("Since")
        },
        first_time.with_timezone(&Local).format(time_format_string),
        (offset / usize::from(OFFSET_INC)) + 1,
        (total_lines / usize::from(OFFSET_INC)) + 1,
        lines.concat()
    ))
}

async fn send_message_with_buttons(
    ctx: &Context,
    interaction: &CommandInteraction,
    user_ids: &[i64],
    username: &Option<String>,
    start_date: Option<DateTime<Utc>>,
) -> CommandResult {
    let now = Utc::now();
    let content = gen_playtime_message(ctx, user_ids, username, start_date, now, 0).await?;

    let button_id = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        let db = data.get::<DB>().unwrap();
        #[allow(clippy::panic)]
        sqlx::query!(r#"INSERT INTO playtime_button(author_id, user_ids, username, start_date, end_date, start_offset) VALUES ($1, $2, $3, $4, $5, 0) RETURNING id"#, i64::try_from(interaction.user.id)?, user_ids, username as _, start_date, now).fetch_one(db).await?.id
    };

    interaction
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new()
                .content(&content)
                .components(create_components(0, &content, button_id, true)),
        )
        .await?;

    // leave buttons disabled for 2 seconds, then send the message again with buttons enabled
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    interaction
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new()
                .components(create_components(0, &content, button_id, false)),
        )
        .await?;

    Ok(())
}

pub fn create_components(
    offset: i32,
    content: &str,
    button_id: i32,
    disabled: bool,
) -> Vec<CreateActionRow> {
    vec![CreateActionRow::Buttons(vec![
        CreateButton::new(format!("playtime:first:{button_id}"))
            .style(ButtonStyle::Primary)
            .label("First")
            .disabled(disabled || offset < 1),
        CreateButton::new(format!("playtime:prev:{button_id}"))
            .style(ButtonStyle::Primary)
            .label("Prev")
            .disabled(disabled || offset < 1),
        CreateButton::new(format!("playtime:next:{button_id}"))
            .style(ButtonStyle::Primary)
            .label("Next")
            .disabled(disabled || content.matches('\n').count() < usize::from(OFFSET_INC) + 2),
    ])]
}
