use crate::util;
use crate::DB;
use chrono::{prelude::*, Duration};
use serenity::client::Context;
use serenity::framework::standard::{macros::command, Args, CommandResult};
use serenity::model::channel::Message;
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
// Takes a single optional argument of a username to get playtime specifically for
#[command]
#[only_in(guilds)]
pub async fn playtime(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
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
        let search = args.rest();

        let mention_regex = regex::Regex::new(r#"^\s*<@!?(\d+?)>\s*$"#).unwrap();
        if let Some(captures) = mention_regex.captures(&search) {
            let user_id = if let Ok(user_id) = u64::from_str(captures.get(1).unwrap().as_str()) {
                user_id
            } else {
                msg.channel_id
                    .say(&ctx.http, "```Invalid mention```")
                    .await?;
                return Ok(());
            };
            if let Some(guild) = ctx.cache.guild(msg.guild_id.unwrap()).await {
                if let Ok(member) = guild.member(ctx, user_id).await {
                    username = member.nick
                }
            }
            if username.is_none() {
                let members = util::collect_members(ctx, msg).await;
                username = match members.get(&user_id) {
                    Some(member) => match &member.nick {
                        Some(nick) => Some(nick.clone()),
                        None => Some(member.user.name.clone()),
                    },
                    None => match ctx.cache.user(user_id).await {
                        Some(user) => Some(user.name),
                        None => Some(ctx.http.get_user(user_id).await.unwrap().name),
                    },
                };
            }
            vec![user_id as i64]
        } else if let Some(user) = util::search_user_id_by_name(ctx, msg, search).await {
            username = Some(user.1);
            vec![user.0 as i64]
        } else {
            msg.channel_id
                .say(&ctx.http, "```Unable to find user```")
                .await?;
            return Ok(());
        }
    };

    // get all rows with a user id in the channel
    let rows = {
        let data = ctx.data.read().await;
        let db = data.get::<DB>().unwrap();
        sqlx::query(r#"SELECT create_date, user_id, game_name FROM user_presence WHERE user_id = any($1) order by create_date"#).bind(user_ids).fetch_all(&*db).await?
    };
    if rows.is_empty() {
        msg.channel_id
            .say(
                &ctx.http,
                format!(
                    "```No recorded playtime{}```",
                    if let Some(username) = username {
                        format!(" for {}", username)
                    } else {
                        String::new()
                    }
                ),
            )
            .await?;
        return Ok(());
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
    let now = Local::now();
    let now = now.with_timezone(now.offset());
    for last in last_user_game.values() {
        if let Some(gametime) = gametimes.get_mut(&last.game) {
            // increment existing game time
            *gametime = *gametime + (now - last.date);
        } else {
            // or insert new entry for first-seen game
            gametimes.insert(last.game.clone(), now - last.date);
        }
    }

    // convert HashMap to Vec so we can sort it by time in descending order
    let mut gametimes: Vec<GameTime> = gametimes
        .iter()
        .map(|(game, time)| GameTime {
            time: *time,
            game: game.clone(),
        })
        .collect();

    if gametimes.is_empty() {
        msg.channel_id
            .say(
                &ctx.http,
                format!(
                    "```No recorded playtime{}```",
                    if let Some(username) = username {
                        format!(" for {}", username)
                    } else {
                        String::new()
                    }
                ),
            )
            .await?;
        return Ok(());
    }

    gametimes.sort_by(|a, b| b.time.cmp(&a.time));
    gametimes.truncate(10); // only show top 10
    let longest_game_name = gametimes.iter().map(|g| g.game.len()).max().unwrap(); // get longest game name so we can pad shorter game names and lineup times

    let mut lines = Vec::with_capacity(gametimes.len());
    for gametime in &gametimes {
        lines.push(format!(
            "{:width$} \u{2014} {}:{:02}\n",
            gametime.game,
            gametime.time.num_hours(),
            gametime.time.num_minutes() % 60,
            width = longest_game_name
        ));
    }

    msg.channel_id
        .say(
            &ctx.http,
            format!(
                "```{} {}\n{}```",
                if let Some(username) = username {
                    format!("{} since", username)
                } else {
                    String::from("Since")
                },
                first_time.with_timezone(now.offset()).format("%b %d, %Y"),
                lines.concat()
            ),
        )
        .await?;

    Ok(())
}
