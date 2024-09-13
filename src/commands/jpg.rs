use crate::error::CommandResult;
use futures::stream::StreamExt;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ExtendedColorType, ImageFormat};
use regex::Regex;
use serenity::all::{CommandInteraction, CreateAttachment};
use serenity::builder::EditInteractionResponse;
use serenity::client::Context;
use std::borrow::Cow;
use std::sync::LazyLock;

#[allow(clippy::unwrap_used)]
static URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^https?:\/\/.+\.(?:jpg|png|jpeg|gif|webp|avif)(\?.*)?$").unwrap()
});

pub async fn jpg(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let mut messages = interaction.channel_id.messages_iter(&ctx).take(30).boxed();
    while let Some(res) = messages.next().await {
        let message = res?;

        let mut dynamic_image_opt: Option<DynamicImage> = None;

        for attachment in message.attachments {
            if let Some(ref mime_type) = attachment.content_type {
                if let Some(image_format) = ImageFormat::from_mime_type(mime_type) {
                    let file_bytes = attachment.download().await?;
                    dynamic_image_opt = Some(image::load_from_memory_with_format(
                        &file_bytes,
                        image_format,
                    )?);
                    break;
                }
            }
        }

        if dynamic_image_opt.is_none() && URL_REGEX.is_match(&message.content) {
            let image_bytes = reqwest::get(message.content).await?.bytes().await?;
            dynamic_image_opt = Some(image::load_from_memory(&image_bytes)?);
        }

        if let Some(mut dynamic_image) = dynamic_image_opt {
            if dynamic_image.width() > 400 || dynamic_image.height() > 400 {
                dynamic_image = dynamic_image.resize(400, 400, FilterType::Nearest);
            }
            let rgb8 = dynamic_image.into_rgb8();
            let mut compressed_jpeg = Vec::new();
            JpegEncoder::new_with_quality(&mut compressed_jpeg, 1).encode(
                &rgb8,
                rgb8.width(),
                rgb8.height(),
                ExtendedColorType::Rgb8,
            )?;
            interaction
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new().new_attachment(CreateAttachment::bytes(
                        Cow::from(compressed_jpeg),
                        "good.jpg",
                    )),
                )
                .await?;
            return Ok(());
        }
    }
    interaction
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new().content("unable to find image"),
        )
        .await?;
    Ok(())
}
