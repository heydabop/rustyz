use crate::model::DB;
use crate::util;
use chrono::{prelude::*, Duration};
use regex::{Match, Regex};
use serenity::builder::CreateComponents;
use serenity::client::Context;
use serenity::framework::standard::{macros::command, Args, CommandResult};
use serenity::model::channel::Message;
use serenity::model::interactions::message_component::ButtonStyle;
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
#[command]
#[only_in(guilds)]
pub async fn playtime(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let (user_ids, username): (Vec<i64>, Option<String>) =
        match user_ids_and_name_from_args(ctx, msg, args.rest()).await? {
            Some(u) => (u.0, u.1),
            None => return Ok(()),
        };

    send_message_with_buttons(ctx, msg, &user_ids, &username, None).await?;

    Ok(())
}

// Replies to msg with the cumulative playtime since the given time period of all users in the guild
// Takes two arguments
// First (required): human readable time duration (2 days, 1 hour, 3 months, etc)
// Second (optional): username to filter playtime for
#[command("recentplaytime")]
#[only_in(guilds)]
pub async fn recent_playtime(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let args = args.rest();
    let duration_regex = Regex::new(r#"(?i)(?:(?:(?:(\d+)\s+years?)|(?:(\d+)\s+months?)|(?:(\d+)\s+weeks?)|(?:(\d+)\s+days?)|(?:(\d+)\s+hours?)|(?:(\d+)\s+minutes?)|(?:(\d+)\s+seconds?))\s?)+(?:\s*(.*))?"#).unwrap();
    let now = Local::now();
    let now = now.with_timezone(now.offset());
    let (start_date, mention): (DateTime<FixedOffset>, String) =
        if let Some(captures) = duration_regex.captures(args) {
            let years = get_digit_from_match(captures.get(1));
            let months = get_digit_from_match(captures.get(2));
            let weeks = get_digit_from_match(captures.get(3));
            let days = get_digit_from_match(captures.get(4));
            let hours = get_digit_from_match(captures.get(5));
            let minutes = get_digit_from_match(captures.get(6));
            let seconds = get_digit_from_match(captures.get(7));
            let since = now
                - Duration::days(years * 365)
                - Duration::days(months_to_days(now, months))
                - Duration::days(weeks * 7)
                - Duration::days(days)
                - Duration::hours(hours)
                - Duration::minutes(minutes)
                - Duration::seconds(seconds);

            let mention = match captures.get(8) {
                Some(c) => c.as_str(),
                None => "",
            };

            (since, String::from(mention))
        } else {
            util::record_say(ctx, msg, "```Unable to parse time```").await?;
            return Ok(());
        };
    let (user_ids, username) = match user_ids_and_name_from_args(ctx, msg, &mention).await? {
        Some(u) => (u.0, u.1),
        None => return Ok(()),
    };

    send_message_with_buttons(ctx, msg, &user_ids, &username, Some(start_date)).await?;

    Ok(())
}

fn get_digit_from_match(mat: Option<Match>) -> i64 {
    match mat {
        None => 0,
        Some(mat) => i64::from_str(mat.as_str()).unwrap(),
    }
}

// takes months and turns it to days by counting the days of each month, supports + or - months
fn months_to_days(now: DateTime<FixedOffset>, mut months: i64) -> i64 {
    let mut end = now;
    loop {
        if months == 0 {
            break (end - now).num_days();
        }
        end = end
            .checked_add_signed(Duration::days(
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
            ))
            .unwrap();
        if months > 0 {
            months -= 1;
        } else {
            months += 1;
        }
    }
}

async fn user_ids_and_name_from_args(
    ctx: &Context,
    msg: &Message,
    args: &str,
) -> CommandResult<Option<(Vec<i64>, Option<String>)>> {
    let mut username: Option<String> = None;
    #[allow(clippy::cast_possible_wrap)]
    let user_ids: Vec<i64> = if args.is_empty() {
        // get list of user IDs in channel
        let guild = match msg.channel(&ctx.cache).await {
            Some(channel) => channel,
            None => ctx.http.get_channel(msg.channel_id.0).await.unwrap(),
        }
        .guild()
        .unwrap();
        let members = match guild.members(&ctx.cache).await {
            Ok(members) => members,
            Err(_) => ctx
                .http
                .get_guild_members(guild.id.0, None, None)
                .await
                .unwrap(),
        };
        members.iter().map(|m| *m.user.id.as_u64() as i64).collect()
    } else {
        let mention_regex = Regex::new(r#"^\s*<@!?(\d+?)>\s*$"#).unwrap();
        if let Some(captures) = mention_regex.captures(args) {
            let user_id = if let Ok(user_id) = u64::from_str(captures.get(1).unwrap().as_str()) {
                user_id
            } else {
                util::record_say(ctx, msg, "```Invalid mention```").await?;
                return Ok(None);
            };
            if let Some(guild) = ctx.cache.guild(msg.guild_id.unwrap()).await {
                if let Ok(member) = guild.member(ctx, user_id).await {
                    username = member.nick;
                }
            }
            if username.is_none() {
                let members = util::collect_members(ctx, msg).await?;
                username = if let Some(member) = members.get(&user_id) {
                    match &member.nick {
                        Some(nick) => Some(nick.clone()),
                        None => Some(member.user.name.clone()),
                    }
                } else {
                    util::record_say(ctx, msg, "```Unable to find user```").await?;
                    return Ok(None);
                };
            }
            vec![user_id as i64]
        } else if let Some(user) = util::search_user_id_by_name(ctx, msg, args).await? {
            username = Some(user.1);
            vec![user.0 as i64]
        } else {
            util::record_say(ctx, msg, "```Unable to find user```").await?;
            return Ok(None);
        }
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
) -> CommandResult<String> {
    // get all rows with a user id in the channel
    let rows = {
        let data = ctx.data.read().await;
        let db = data.get::<DB>().unwrap();
        sqlx::query(r#"SELECT create_date, user_id, game_name FROM user_presence WHERE user_id = any($1) AND (create_date > $2) IS NOT FALSE AND create_date <= $3 ORDER BY create_date"#).bind(user_ids).bind(start_date).bind(end_date).fetch_all(&*db).await?
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

        let last = last_user_game.get(&user_id);
        // user wasn't playing anything, record new entry if user is playing something now, otherwise just continue
        if last.is_none() {
            if let Some(game) = game {
                last_user_game.insert(user_id, GameDate { date, game });
            }
            continue;
        }

        let last = last.unwrap();
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
    let mut gametimes: Vec<GameTime> = gametimes
        .iter()
        .map(|(game, time)| {
            total_time = total_time.checked_add(time).unwrap();
            GameTime {
                time: *time,
                game: game.clone(),
            }
        })
        .collect();

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
    let longest_game_name = gametimes.iter().map(|g| g.game.len()).max().unwrap(); // get longest game name so we can pad shorter game names and lineup times

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
    msg: &Message,
    user_ids: &[i64],
    username: &Option<String>,
    start_date: Option<DateTime<FixedOffset>>,
) -> CommandResult<()> {
    let now = Local::now();
    let now = now.with_timezone(now.offset());
    let content = gen_playtime_message(ctx, user_ids, username, start_date, now, 0).await?;

    #[allow(clippy::cast_possible_wrap)]
    let insert = {
        let data = ctx.data.read().await;
        let db = data.get::<DB>().unwrap();
        sqlx::query(r#"INSERT INTO playtime_button(author_id, user_ids, username, start_date, end_date, start_offset) VALUES ($1, $2, $3, $4, $5, 0) RETURNING id"#).bind(msg.author.id.0 as i64).bind(user_ids).bind(username).bind(start_date).bind(now).fetch_one(&*db).await?
    };
    let button_id = insert.get::<i32, _>(0);

    let reply = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.content(&content);
            m.components(|c| create_components(c, 0, &content, button_id));
            m
        })
        .await?;

    util::record_sent_message(ctx, msg, reply.id).await;

    Ok(())
}

pub fn create_components<'a>(
    components: &'a mut CreateComponents,
    offset: i32,
    content: &str,
    button_id: i32,
) -> &'a mut CreateComponents {
    components.create_action_row(|a| {
        a.create_button(|b| {
            b.custom_id(format!("playtime:prev:{}", button_id))
                .style(ButtonStyle::Primary)
                .label("Prev")
                .disabled(offset < 1);
            b
        });
        a.create_button(|b| {
            b.custom_id(format!("playtime:next:{}", button_id))
                .style(ButtonStyle::Primary)
                .label("Next")
                .disabled(content.matches('\n').count() < 17);
            b
        });
        a
    });
    components
}