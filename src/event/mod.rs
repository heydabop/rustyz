mod interaction;
mod message;
mod presence;

use crate::model;

use serde_json::json;
use serenity::all::{
    Command, CommandDataOptionValue, CommandInteraction, CommandOptionType, Interaction,
};
use serenity::async_trait;
use serenity::builder::{CreateCommand, CreateCommandOption};
use serenity::client::{Context, EventHandler};
use serenity::json::Value;
use serenity::model::{
    channel::Message,
    event::MessageUpdateEvent,
    gateway::{Presence, Ready},
    guild::{Guild, Member, UnavailableGuild},
    id::{ChannelId, GuildId, MessageId, UserId},
    user::User,
};
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use tracing::{error, info, warn};

pub struct Handler {
    db: Pool<Postgres>,
    twitch_regex: regex::Regex,
    vote_regex: regex::Regex,
}

impl Handler {
    pub fn new(db: Pool<Postgres>) -> Result<Self, regex::Error> {
        #[allow(clippy::unwrap_used)]
        Ok(Self {
            db,
            twitch_regex: regex::RegexBuilder::new(r"https?://(www\.)?twitch.tv/(\w+)")
                .case_insensitive(true)
                .build()?,
            vote_regex: regex::RegexBuilder::new(r"<@!?(\d+?)>\s*(\+\+|--)").build()?,
        })
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("Bot {} is successfully connected.", ready.user.name);

        #[allow(clippy::unreadable_literal)]
        let g = GuildId::new(184428741450006528);
        match g
            .set_commands(
                &ctx.http,
                vec![
                    CreateCommand::new("birdtime").description("Sends the current time for bird"),
                    CreateCommand::new("mirotime").description("Sends the current time for miro"),
                    CreateCommand::new("nieltime").description("Sends the current time for niel"),
                    CreateCommand::new("realtime")
                        .description("Sends the current time for the mainlanders"),
                    CreateCommand::new("sebbitime").description("Sends the current time for sebbi"),
                ],
            )
            .await
        {
            Ok(guild_commands) => info!(guild_commands = ?guild_commands
                    .iter()
                    .map(|g| &g.name)
                    .collect::<Vec<&String>>(),
                "guild commands set",
            ),
            Err(e) => error!(%e, "error setting guild commands"),
        }

        match Command::set_global_commands(&ctx.http, vec![
            CreateCommand::new("affixes").description("Sends this week's US Mythic+ affixes"),
            //CreateCommand::new("asuh").description("Joins your voice channel and plays bothersome audio"),
            CreateCommand::new("botinfo").description("Displays details about the bot"),
            CreateCommand::new("downvote").description("Downvote a user (lowering their karma by one)")
                .add_option(CreateCommandOption::new(CommandOptionType::User, "user", "User to downvote")
                            .required(true)),
            CreateCommand::new("forecast")
                .description("Sends hourly weather conditions over the next 12 hours for an area")
                .set_options(vec![
                    CreateCommandOption::new(CommandOptionType::String, "location", "Area to get weather for; can be city name, postal code, or decimal lat/long (default: Austin, TX)"),
                    CreateCommandOption::new(CommandOptionType::Integer, "hours", "How many hours into the future to forecast (default: 6)")
                        .min_int_value(2)
                        .max_int_value(12)
                ]),
            CreateCommand::new("fortune").description("Sends a random adage"),
            CreateCommand::new("jpg")
                .description("Efficiently compresses the most recently posted image")
                .add_option(
                    CreateCommandOption::new(CommandOptionType::Attachment, "image", "Or directly upload an image to be efficiently compressed")),
            CreateCommand::new("serverinfo").description("Displays details about this server"),
            CreateCommand::new("invite").description("Generates link to add bot to a server you administrate"),
            CreateCommand::new("karma").description("Lists members by karma points")
                .add_option(
                    CreateCommandOption::new(CommandOptionType::Integer, "count", "The number of members to list (defaults to 5)")
                        .min_int_value(1)
                        .max_int_value(100)
                ),
            CreateCommand::new("lastseen")
                .description("Sends how long it's been since a user was last online")
                .add_option(CreateCommandOption::new(CommandOptionType::User, "user", "User to check")
                            .required(true)),
            CreateCommand::new("lastplayed")
                .description("How long it's been since a user was last playing a game, and the game they were playing")
                .add_option(CreateCommandOption::new(CommandOptionType::User, "user", "User to check")
                            .required(true)),
            CreateCommand::new("math")
                .description("Does math (with Wolfram Alpha)")
                .add_option(CreateCommandOption::new(CommandOptionType::String, "question", "A question; answerable by text")
                            .required(true)),
            CreateCommand::new("ping").description("pong"),
            CreateCommand::new("playtime")
                .description("Shows all recorded video game playtime of a user or everyone in this server")
                .add_option(CreateCommandOption::new(CommandOptionType::User, "user", "User to show playtime for")),
            CreateCommand::new("raiderio")
                .description("Displays raider.io stats for given character")
                .set_options(vec![
                    CreateCommandOption::new(CommandOptionType::String, "character", "Character to get stats for")
                        .required(true),
                    CreateCommandOption::new(CommandOptionType::String, "realm", "Realm that character is on")
                        .required(true)
                ]),
            CreateCommand::new("recentplaytime")
                .description("Shows video game playtime over a specified duration of a user or everyone in this server")
                .set_options(vec![
                    CreateCommandOption::new(CommandOptionType::String, "duration", "Duration to show playtime for (1 week, 2 months, etc)")
                        .required(true),
                    CreateCommandOption::new(CommandOptionType::User, "user", "User to show playtime for")
                ]),
            CreateCommand::new("roll")
                .description("Roll a die")
                .add_option(CreateCommandOption::new(CommandOptionType::Integer, "sides", "Sides on die (default 100)")
                            .min_int_value(1)
                            .max_int_value(u32::MAX.into())),
            CreateCommand::new("source").description("Sends link to bot source code"),
            /*CreateCommand::new("tarkov")
            .description("Sends flea market and vendor info for item")
            .create_option(|o| {
            o.name("item")
            .description("Tarkov item to search the flea market for")
            .kind(CommandOptionType::String)
            .required(true)
        }),
        })*/
            CreateCommand::new("top")
                .description("Lists members by number of sent messages")
                .add_option(CreateCommandOption::new(CommandOptionType::Integer, "count", "The number of members to list (defaults to 5)")
                            .min_int_value(1)
                            .max_int_value(100)),
            CreateCommand::new("topcommand")
                .description("Lists members by most command invocations")
                .add_option(CreateCommandOption::new(CommandOptionType::String, "command", "Command to list invocations for")
                            .required(true)),
            CreateCommand::new("toplength")
                .description("Lists members by average length of sent messages")
                .add_option(CreateCommandOption::new(CommandOptionType::Integer, "count", "The number of members to list (defaults to 5)")
                            .min_int_value(1)
                            .max_int_value(100)),
            CreateCommand::new("track")
                .description("Track shipment")
                .set_options(vec![
                    CreateCommandOption::new(CommandOptionType::String, "carrier", "Shipping company")
                        .required(true)
                        .add_string_choice("FedEx", "fedex")
                        .add_string_choice("UPS", "ups")
                        .add_string_choice("USPS", "usps"),
                    CreateCommandOption::new(CommandOptionType::String, "number", "Tracking number")
                        .required(true),
                    CreateCommandOption::new(CommandOptionType::String, "comment", "Optional comment descriping shipment, will be sent to channel upon package delivery")
                ]),
            CreateCommand::new("userinfo").description("Displays details about a user")
                .add_option(CreateCommandOption::new(CommandOptionType::User, "user", "User to display")
                            .required(true)),
            CreateCommand::new("upvote").description("Upvote a user (increasing their karma by one)")
                .add_option(CreateCommandOption::new(CommandOptionType::User, "user", "User to upvote")
                            .required(true)),
            CreateCommand::new("weather")
                .description("Sends weather conditions for an area")
                .add_option(CreateCommandOption::new(CommandOptionType::String, "location", "Area to get weather for; can be city name, postal code, or decimal lat/long (default: Austin, TX)")),
            CreateCommand::new("whois")
                .description("Lookup username by ID")
                .add_option(CreateCommandOption::new(CommandOptionType::String, "id", "ID of user to find")
                            .required(true)),
            CreateCommand::new("wolframalpha")
                .description("Queries Wolfram Alpha and returns an image result")
                .add_option(CreateCommandOption::new(CommandOptionType::String, "input", "Input query")
                            .required(true)),
            CreateCommand::new("wow")
                .description("World of Warcraft commands")
                .set_options(vec![
                    CreateCommandOption::new(CommandOptionType::SubCommand, "character", "WoW character details")
                        .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "character", "Character name")
                                        .required(true))
                        .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "realm", "Character's realm")
                                        .required(true)),
                    CreateCommandOption::new(CommandOptionType::SubCommand, "realm", "Status of WoW realm").add_sub_option(CreateCommandOption::new(CommandOptionType::String, "realm", "Realm name")),
                    CreateCommandOption::new(CommandOptionType::SubCommand, "search", "Search all realms for WoW character by name")
                        .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "character", "Character name").required(true)),
                    CreateCommandOption::new(CommandOptionType::SubCommand, "transmog", "Image of character from WoW armory")
                        .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "character", "Character name")
                                        .required(true))
                        .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "realm", "Character's realm")
                                        .required(true))
                ]),
            CreateCommand::new("zalgo")
                .description("HE COMES")
                .add_option(CreateCommandOption::new(CommandOptionType::String, "message", "HE COMES")
                            .required(true)),
        ])
        .await {
            Ok(commands) => info!(commands = ?commands.iter().map(|g| &g.name).collect::<Vec<&String>>(), "commands set"),
            Err(e) => error!(%e, "error setting commands"),
        }
    }

    async fn presence_update(&self, ctx: Context, update: Presence) {
        presence::update(&ctx, &self.db, update.guild_id, update, false).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let db = self.db.clone();
        // TODO? tokio::spawn
        interaction::create(ctx, db, interaction).await;
    }

    async fn guild_create(&self, ctx: Context, guild: Guild, is_new: Option<bool>) {
        if is_new.unwrap_or(false) {
            info!(
                id = guild.id.get(),
                members = guild.member_count,
                name = guild.name,
                "joined guild"
            );
        }
        let is_startup = match is_new {
            Some(true) => false,
            Some(false) | None => true,
        };
        for (_, presence) in guild.presences {
            presence::update(&ctx, &self.db, Some(guild.id), presence, is_startup).await;
        }
    }

    async fn guild_delete(&self, _: Context, guild: UnavailableGuild, _: Option<Guild>) {
        warn!(
            id = guild.id.get(),
            offline = guild.unavailable,
            "left guild"
        );
    }

    async fn guild_member_removal(
        &self,
        ctx: Context,
        guild_id: GuildId,
        user: User,
        _: Option<Member>,
    ) {
        // Check to see if we can still see this user in other guilds, if not mark them as offline in DB
        let guild_lists = {
            let data = ctx.data.read().await;
            #[allow(clippy::unwrap_used)]
            data.get::<model::UserGuildList>().unwrap().clone()
        };
        let is_empty = {
            let mut guild_lists = guild_lists.write().await;
            match guild_lists.get_mut(&user.id) {
                Some(user_list) => {
                    // Remove this guild ID from user's list and check if list is now empty
                    user_list.remove(&guild_id);
                    user_list.is_empty()
                }
                None => true,
            }
        };
        if is_empty {
            #[allow(clippy::panic)]
            if let Err(e) = sqlx::query!(
                r"INSERT INTO user_presence (user_id, status) VALUES ($1, 'offline'::online_status)",
                i64::from(user.id))
                .execute(&self.db)
                .await
            {
                error!(%e, "Error saving user_presence");
            }
        }
    }

    async fn message(&self, ctx: Context, msg: Message) {
        message::create(self, ctx, msg).await;
    }

    async fn message_delete(
        &self,
        _ctx: Context,
        channel_id: ChannelId,
        message_id: MessageId,
        _guild_id: Option<GuildId>,
    ) {
        message::delete(&self.db, channel_id, message_id).await;
    }

    async fn message_delete_bulk(
        &self,
        _ctx: Context,
        channel_id: ChannelId,
        message_ids: Vec<MessageId>,
        _guild_id: Option<GuildId>,
    ) {
        message::delete_bulk(&self.db, channel_id, message_ids).await;
    }

    async fn message_update(
        &self,
        _ctx: Context,
        _old: Option<Message>,
        _new: Option<Message>,
        update: MessageUpdateEvent,
    ) {
        message::update(&self.db, update).await;
    }
}

async fn report_interaction_error(ctx: &Context, error: String) {
    let owner_id = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        UserId::from(*data.get::<model::OwnerId>().unwrap())
    };
    let channel = match owner_id.create_dm_channel(&ctx.http).await {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "error creating owner DM channel");
            return;
        }
    };
    if let Err(e) = channel.say(&ctx.http, error).await {
        error!(error = %e, "error messaging owner DM channel");
    }
}

async fn record_command(db: &Pool<Postgres>, command: &CommandInteraction) {
    let mut command_name = command.data.name.clone();
    let command_data_options: HashMap<&String, &CommandDataOptionValue> =
        if let Some(option) = command.data.options.first() {
            if let CommandDataOptionValue::SubCommand(suboptions) = &option.value {
                command_name = format!("{command_name} {}", option.name);
                suboptions.iter().map(|o| (&o.name, &o.value)).collect()
            } else {
                command
                    .data
                    .options
                    .iter()
                    .map(|o| (&o.name, &o.value))
                    .collect()
            }
        } else {
            HashMap::new()
        };
    let log_options: HashMap<&String, Value> = command_data_options
        .into_iter()
        .map(|(name, option)| {
            use CommandDataOptionValue::*;
            let value = match option {
                Boolean(b) => json!(b),
                Integer(i) => json!(i),
                Number(n) => json!(n),
                String(s) => json!(s),
                Attachment(a) => json!(a.get()),
                Channel(c) => json!(c.get()),
                Mentionable(m) => json!(m.get()),
                Role(r) => json!(r.get()),
                User(u) => json!(u.get()),
                _ => json!(null),
            };
            (name, value)
        })
        .collect();
    info!(name = command_name, options = ?log_options, "command called");
    #[allow(clippy::panic)]
    if let Err(e) = sqlx::query!(
        r#"
INSERT INTO command(author_id, channel_id, guild_id, name, options)
VALUES ($1, $2, $3, $4, $5)"#,
        i64::from(command.user.id),
        i64::from(command.channel_id),
        command.guild_id.map(i64::from),
        command_name,
        json!(log_options)
    )
    .execute(db)
    .await
    {
        error!(%e, "error inserting command log into db");
    }
}
