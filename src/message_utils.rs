use crate::constants::{self, DESCRIPTION_MAX_LENGTH};
use poise::{
    serenity_prelude::{CreateEmbed, CreateMessage, EditMessage, EMBED_MAX_LENGTH},
    CreateReply,
};
use std::mem;

pub fn success_embed(content: impl Into<String>) -> CreateEmbed {
    CreateEmbed::new()
        .description(content)
        .color(constants::SUCCESS_COLOR)
}

pub fn failure_embed(content: impl Into<String>) -> CreateEmbed {
    CreateEmbed::new()
        .description(content)
        .color(constants::FAILURE_COLOR)
}

pub fn info_embed(content: impl Into<String>) -> CreateEmbed {
    CreateEmbed::new()
        .description(content)
        .color(constants::INFO_COLOR)
}

pub fn success_message(content: impl Into<String>) -> CreateReply {
    CreateReply::default().embed(success_embed(content))
}

#[allow(dead_code)]
pub fn failure_message(content: impl Into<String>) -> CreateReply {
    CreateReply::default().embed(failure_embed(content))
}

#[allow(dead_code)]
pub fn info_message(content: impl Into<String>) -> CreateReply {
    CreateReply::default().embed(info_embed(content))
}

pub fn render_lines_reply<S: Into<String>, T: Into<String>>(
    lines: impl IntoIterator<Item = S>,
    title: impl Into<Option<T>>,
) -> CreateReply {
    let title: Option<String> = title.into().map(Into::into);
    let remaining_chars = EMBED_MAX_LENGTH - title.as_ref().map_or_else(|| 1, String::len) + 2;
    let mut lines = lines
        .into_iter()
        .map(|s| {
            let mut s: String = s.into();
            s.push('\n');
            s
        })
        .collect::<Vec<String>>();
    let lines_dropped = {
        let lines_before_drop = lines.len();
        lines.retain(|s| s.len() <= DESCRIPTION_MAX_LENGTH + 1);
        lines_before_drop - lines.len()
    };
    lines.sort_unstable_by_key(String::len);
    let mut chars_dropped = 0usize;
    let mut total_chars = lines.iter().fold(0, |total_chars, s| total_chars + s.len());
    while total_chars > remaining_chars {
        let chars = lines.pop().unwrap().len();
        total_chars -= chars;
        chars_dropped += chars;
    }
    if total_chars <= 4097 {
        let mut description = lines.concat();
        description.pop();
        let mut embed = info_embed(description);
        if let Some(title) = title {
            embed = embed.title(title);
        }
        CreateReply::default().embed(embed)
    } else {
        let mut half_lines = (lines.len() + 1) / 2;
        let mut first_description = String::new();
        lines.retain(|line| {
            if half_lines > 0 && first_description.len() + line.len() <= DESCRIPTION_MAX_LENGTH + 1
            {
                half_lines -= 1;
                first_description.push_str(line);
                true
            } else {
                false
            }
        });
        let mut second_description = lines.concat();
        first_description.pop();
        second_description.pop();
        if half_lines > 0 {
            mem::swap(&mut first_description, &mut second_description);
        }
        {
            let mut first_embed = info_embed(first_description);
            if let Some(title) = title {
                first_embed = first_embed.title(title);
            }
            let res = CreateReply::default()
                .embed(first_embed)
                .embed(info_embed(second_description));
            if lines_dropped > 0 {
                res.content(format!(
                "This output has been truncated by {lines_dropped} lines ({chars_dropped} characters) because of Discord limits."
            ))
            } else {
                res
            }
        }
    }
}

pub fn render_lines_message<S: Into<String>, T: Into<String>>(
    content: impl Into<String>,
    lines: impl IntoIterator<Item = S>,
    title: impl Into<Option<T>>,
) -> CreateMessage {
    let title: Option<String> = title.into().map(Into::into);
    let remaining_chars = EMBED_MAX_LENGTH - title.as_ref().map_or_else(|| 1, String::len) + 2;
    let mut lines = lines
        .into_iter()
        .map(|s| {
            let mut s: String = s.into();
            s.push('\n');
            s
        })
        .collect::<Vec<String>>();
    let lines_dropped = {
        let lines_before_drop = lines.len();
        lines.retain(|s| s.len() <= DESCRIPTION_MAX_LENGTH + 1);
        lines_before_drop - lines.len()
    };
    lines.sort_unstable_by_key(String::len);
    let mut chars_dropped = 0usize;
    let mut total_chars = lines.iter().fold(0, |total_chars, s| total_chars + s.len());
    while total_chars > remaining_chars {
        let chars = lines.pop().unwrap().len();
        total_chars -= chars;
        chars_dropped += chars;
    }
    if total_chars <= 4097 {
        let mut description = lines.concat();
        description.pop();
        let mut embed = info_embed(description);
        if let Some(title) = title {
            embed = embed.title(title);
        }
        CreateMessage::default().content(content).embed(embed)
    } else {
        let mut half_lines = (lines.len() + 1) / 2;
        let mut first_description = String::new();
        lines.retain(|line| {
            if half_lines > 0 && first_description.len() + line.len() <= DESCRIPTION_MAX_LENGTH + 1
            {
                half_lines -= 1;
                first_description.push_str(line);
                true
            } else {
                false
            }
        });
        let mut second_description = lines.concat();
        first_description.pop();
        second_description.pop();
        if half_lines > 0 {
            mem::swap(&mut first_description, &mut second_description);
        }
        {
            let mut first_embed = info_embed(first_description);
            if let Some(title) = title {
                first_embed = first_embed.title(title);
            }
            let res = CreateMessage::default()
                .embed(first_embed)
                .embed(info_embed(second_description));
            if lines_dropped > 0 {
                res.content(format!(
                    "{}\nThis output has been truncated by {lines_dropped} lines ({chars_dropped} characters) because of Discord limits.", content.into()
                ))
            } else {
                res.content(content)
            }
        }
    }
}

pub fn render_lines_edit_message<S: Into<String>, T: Into<String>>(
    content: impl Into<String>,
    lines: impl IntoIterator<Item = S>,
    title: impl Into<Option<T>>,
) -> EditMessage {
    let title: Option<String> = title.into().map(Into::into);
    let remaining_chars = EMBED_MAX_LENGTH - title.as_ref().map_or_else(|| 1, String::len) + 2;
    let mut lines = lines
        .into_iter()
        .map(|s| {
            let mut s: String = s.into();
            s.push('\n');
            s
        })
        .collect::<Vec<String>>();
    let lines_dropped = {
        let lines_before_drop = lines.len();
        lines.retain(|s| s.len() <= DESCRIPTION_MAX_LENGTH + 1);
        lines_before_drop - lines.len()
    };
    lines.sort_unstable_by_key(String::len);
    let mut chars_dropped = 0usize;
    let mut total_chars = lines.iter().fold(0, |total_chars, s| total_chars + s.len());
    while total_chars > remaining_chars {
        let chars = lines.pop().unwrap().len();
        total_chars -= chars;
        chars_dropped += chars;
    }
    if total_chars <= 4097 {
        let mut description = lines.concat();
        description.pop();
        let mut embed = info_embed(description);
        if let Some(title) = title {
            embed = embed.title(title);
        }
        EditMessage::default().content(content).embed(embed)
    } else {
        let mut half_lines = (lines.len() + 1) / 2;
        let mut first_description = String::new();
        lines.retain(|line| {
            if half_lines > 0 && first_description.len() + line.len() <= DESCRIPTION_MAX_LENGTH + 1
            {
                half_lines -= 1;
                first_description.push_str(line);
                true
            } else {
                false
            }
        });
        let mut second_description = lines.concat();
        first_description.pop();
        second_description.pop();
        if half_lines > 0 {
            mem::swap(&mut first_description, &mut second_description);
        }
        {
            let mut first_embed = info_embed(first_description);
            if let Some(title) = title {
                first_embed = first_embed.title(title);
            }
            let res = EditMessage::default()
                .embed(first_embed)
                .embed(info_embed(second_description));
            if lines_dropped > 0 {
                res.content(format!(
                    "{}\nThis output has been truncated by {lines_dropped} lines ({chars_dropped} characters) because of Discord limits.", content.into()
                ))
            } else {
                res.content(content)
            }
        }
    }
}
