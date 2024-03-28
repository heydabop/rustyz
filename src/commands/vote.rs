use crate::error::{CommandError, CommandResult};
use crate::model::DB;
use chrono::{prelude::*, Duration};
use rand::{thread_rng, Rng};
use serenity::all::{CommandDataOptionValue, CommandInteraction, UserId};
use serenity::builder::EditInteractionResponse;
use serenity::client::Context;
use serenity::model::id::GuildId;
use sqlx::PgPool;

pub async fn vote_from_interaction(
    ctx: &Context,
    interaction: &CommandInteraction,
    is_upvote: bool,
) -> CommandResult {
    let Some(guild_id) = interaction.guild_id else {
        interaction
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new().content("Command can only be used in a server"),
            )
            .await?;
        return Ok(());
    };
    let user_id = if let Some(o) = interaction.data.options.first() {
        if let CommandDataOptionValue::User(u) = o.value {
            u
        } else {
            interaction
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new().content("Unable to find user"),
                )
                .await?;
            return Ok(());
        }
    } else {
        interaction
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new().content("Unable to find user"),
            )
            .await?;
        return Ok(());
    };
    if let Some(reply) =
        process_vote(ctx, is_upvote, interaction.user.id, guild_id, user_id).await?
    {
        interaction
            .edit_response(&ctx.http, EditInteractionResponse::new().content(reply))
            .await?;
    }
    Ok(())
}

pub async fn process_vote(
    ctx: &Context,
    is_upvote: bool,
    author_id: UserId,
    guild_id: GuildId,
    user_id: UserId,
) -> Result<Option<&'static str>, CommandError> {
    let author_id = i64::from(author_id);
    let guild_id = i64::from(guild_id);
    let user_id = i64::from(user_id);
    let db = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        data.get::<DB>().unwrap().clone()
    };

    if author_id == user_id && is_upvote {
        record_vote(db, false, author_id, guild_id, author_id).await?;
        return Ok(Some("No."));
    }

    #[allow(clippy::panic)]
    let last_vote_time: DateTime<Utc> = sqlx::query!(
        "SELECT MAX(create_date) AS last FROM vote WHERE voter_id = $1",
        author_id
    )
    .fetch_one(&db)
    .await?
    .last
    .unwrap_or_default();

    let now = Utc::now();
    #[allow(clippy::cast_possible_truncation, clippy::unwrap_used)]
    if now.signed_duration_since(last_vote_time)
        < Duration::try_seconds((300.0 + (300.0 * thread_rng().gen::<f64>())) as i64).unwrap()
    {
        return Ok(Some("Slow down champ."));
    }

    #[allow(clippy::panic, clippy::unwrap_used)]
    if let Some(last_vote_against) = sqlx::query!(
        "SELECT voter_id, create_date FROM vote WHERE guild_id = $1 AND votee_id = $2 ORDER BY create_date DESC LIMIT 1",
        guild_id,
        author_id)
        .fetch_optional(&db).await? {
            if last_vote_against.voter_id == user_id && now.signed_duration_since(last_vote_against.create_date) < Duration::try_hours(12).unwrap() {
                return Ok(Some("Really?..."));
            }
        }
    #[allow(clippy::panic, clippy::unwrap_used)]
    if let Some(last_vote_from) = sqlx::query!(
        "SELECT votee_id, create_date FROM vote WHERE guild_id = $1 AND voter_id = $2 ORDER BY create_date DESC LIMIT 1",
        guild_id,
        author_id)
        .fetch_optional(&db).await? {
            if last_vote_from.votee_id == user_id && now.signed_duration_since(last_vote_from.create_date) < Duration::try_hours(12).unwrap() {
                return Ok(Some("Really?..."));
            }
        }

    record_vote(db, is_upvote, author_id, guild_id, user_id).await?;

    Ok(None)
}

#[allow(clippy::similar_names)]
pub async fn record_vote(
    db: PgPool,
    is_upvote: bool,
    voter_id: i64,
    guild_id: i64,
    votee_id: i64,
) -> Result<(), sqlx::Error> {
    #[allow(clippy::panic)]
    sqlx::query!(
        "INSERT INTO vote(guild_id, voter_id, votee_id, is_upvote) VALUES ($1, $2, $3, $4)",
        guild_id,
        voter_id,
        votee_id,
        is_upvote
    )
    .execute(&db)
    .await?;

    #[allow(clippy::panic)]
    sqlx::query!(
        "INSERT INTO user_karma(guild_id, user_id, karma) VALUES ($1, $2, $3)
         ON CONFLICT ON CONSTRAINT user_karma_pkey DO UPDATE SET karma = user_karma.karma + $3",
        guild_id,
        votee_id,
        if is_upvote { 1 } else { -1 },
    )
    .execute(&db)
    .await?;
    Ok(())
}
