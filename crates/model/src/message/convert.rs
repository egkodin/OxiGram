//! Messages out of TDLib's types.

use super::*;

/// Views, replies and reactions out of `messageInteractionInfo`.
pub(crate) fn interaction(
    info: Option<Box<td_api::types::MessageInteractionInfo>>,
) -> (i32, Option<Replies>, Option<Reactions>) {
    info.map_or((0, None, None), |info| {
        (
            info.view_count,
            info.reply_info.map(|replies| Replies::from(*replies)),
            info.reactions.map(|reactions| Reactions::from(*reactions)),
        )
    })
}

impl From<td_api::types::Message> for Message {
    fn from(message: td_api::types::Message) -> Self {
        let (views, replies, reactions) = interaction(message.interaction_info);
        Self {
            id: MessageId(message.id),
            chat: ChatId(message.chat_id),
            sender: Sender::from(*message.sender_id),
            is_outgoing: message.is_outgoing,
            // TDLib dates scheduled messages 0; they show their send date.
            date: match message.scheduling_state.as_deref() {
                Some(MessageSchedulingState::SendAtDate(state)) => state.send_date,
                _ => message.date,
            },
            edit_date: message.edit_date,
            content: Content::from(*message.content),
            reply_to: message.reply_to.map(|reply| ReplyTo::from_td(*reply)),
            forward: message.forward_info.map(|forward| Forward {
                origin: Origin::from(*forward.origin),
                date: forward.date,
            }),
            sending: message.sending_state.map(|state| match *state {
                MessageSendingState::Pending(_) => Sending::Pending,
                MessageSendingState::Failed(_) => Sending::Failed,
            }),
            views,
            replies,
            reactions,
            topic: message.topic_id.and_then(|topic| Topic::from_td(*topic)),
            author_signature: message.author_signature,
            album: Some(message.media_album_id).filter(|album| *album != 0),
            is_channel_post: message.is_channel_post,
            via_bot: Some(UserId(message.via_bot_user_id)).filter(|bot| bot.0 != 0),
            can_be_saved: message.can_be_saved,
            schedule: message
                .scheduling_state
                .map(|state| Schedule::from_td(*state)),
            self_destruct: message
                .self_destruct_type
                .map(|kind| SelfDestruct::from(*kind)),
        }
    }
}

impl ReplyTo {
    fn from_td(reply: MessageReplyTo) -> Self {
        match reply {
            MessageReplyTo::Message(reply) => Self {
                chat: ChatId(reply.chat_id),
                message: MessageId(reply.message_id),
                quote: reply.quote.map(|quote| FormattedText::from(*quote.text)),
                external: reply.origin.map(|origin| {
                    Box::new(External::Message {
                        origin: Origin::from(*origin),
                        media: reply
                            .content
                            .map(|content| Box::new(Content::from(*content))),
                    })
                }),
            },
            MessageReplyTo::Story(story) => Self {
                chat: ChatId(story.story_poster_chat_id),
                message: MessageId(0),
                quote: None,
                external: Some(Box::new(External::Story {
                    poster: ChatId(story.story_poster_chat_id),
                })),
            },
        }
    }
}

impl From<MessageOrigin> for Origin {
    fn from(origin: MessageOrigin) -> Self {
        match origin {
            MessageOrigin::User(user) => Self::User(UserId(user.sender_user_id)),
            MessageOrigin::HiddenUser(user) => Self::HiddenUser(user.sender_name),
            MessageOrigin::Chat(chat) => Self::Chat {
                chat: ChatId(chat.sender_chat_id),
                signature: chat.author_signature,
            },
            MessageOrigin::Channel(channel) => Self::Channel {
                chat: ChatId(channel.chat_id),
                message: MessageId(channel.message_id),
                signature: channel.author_signature,
            },
        }
    }
}

impl From<MessageSender> for Sender {
    fn from(sender: MessageSender) -> Self {
        match sender {
            MessageSender::User(user) => Self::User(UserId(user.user_id)),
            MessageSender::Chat(chat) => Self::Chat(ChatId(chat.chat_id)),
        }
    }
}

fn media(
    caption: td_api::types::FormattedText,
    thumbnail: Option<Box<td_api::types::Minithumbnail>>,
    file: Option<td_api::types::File>,
    dimensions: Option<Dimensions>,
) -> Media {
    Media {
        caption: FormattedText::from(caption),
        thumbnail: thumbnail.and_then(|thumbnail| Minithumbnail::from_td(*thumbnail)),
        dimensions,
        preview: None,
        duration: 0,
        file: file.map(File::from),
        has_spoiler: false,
        start_timestamp: 0,
    }
}

/// The thumbnail's file when it is a still image gpui can draw.
pub(crate) fn still(thumbnail: Option<Box<td_api::types::Thumbnail>>) -> Option<File> {
    let thumbnail = thumbnail?;
    matches!(*thumbnail.format, td_api::enums::ThumbnailFormat::Jpeg(_))
        .then(|| File::from(*thumbnail.file))
}

impl From<MessageContent> for Content {
    fn from(content: MessageContent) -> Self {
        match content {
            MessageContent::MessageText(text) => Self::Text {
                text: FormattedText::from(*text.text),
                preview: text
                    .link_preview
                    .map(|preview| Box::new(LinkPreview::from(*preview))),
            },
            MessageContent::MessagePhoto(photo) => {
                let largest = photo.photo.sizes.into_iter().last();
                let dimensions = largest
                    .as_ref()
                    .and_then(|size| Dimensions::new(size.width, size.height));
                let file = largest.map(|size| *size.photo);
                Self::Photo(Media {
                    has_spoiler: photo.has_spoiler,
                    ..media(*photo.caption, photo.photo.minithumbnail, file, dimensions)
                })
            }
            MessageContent::MessageVideo(video) => {
                // A cover the sender picked stands in for TDLib's still
                // (`LookupVideoCover`).
                let (preview, thumbnail) = match video.cover {
                    Some(cover) => (
                        cover
                            .sizes
                            .into_iter()
                            .last()
                            .map(|size| File::from(*size.photo)),
                        cover.minithumbnail,
                    ),
                    None => (still(video.video.thumbnail), video.video.minithumbnail),
                };
                Self::Video(Media {
                    preview,
                    has_spoiler: video.has_spoiler,
                    duration: video.video.duration,
                    start_timestamp: video.start_timestamp,
                    ..media(
                        *video.caption,
                        thumbnail,
                        Some(*video.video.video),
                        Dimensions::new(video.video.width, video.video.height),
                    )
                })
            }
            MessageContent::MessageAnimation(animation) => Self::Animation(Media {
                preview: still(animation.animation.thumbnail),
                has_spoiler: animation.has_spoiler,
                duration: animation.animation.duration,
                ..media(
                    *animation.caption,
                    animation.animation.minithumbnail,
                    Some(*animation.animation.animation),
                    Dimensions::new(animation.animation.width, animation.animation.height),
                )
            }),
            MessageContent::MessageDocument(document) => Self::Document {
                media: media(
                    *document.caption,
                    document.document.minithumbnail,
                    Some(*document.document.document),
                    None,
                ),
                name: document.document.file_name,
            },
            MessageContent::MessageAudio(audio) => Self::Audio {
                music: Box::new(Music {
                    title: audio.audio.title,
                    performer: audio.audio.performer,
                    duration: audio.audio.duration,
                    file: File::from(*audio.audio.audio),
                }),
                name: audio.audio.file_name,
                caption: FormattedText::from(*audio.caption),
            },
            MessageContent::MessageVoiceNote(voice) => Self::Voice {
                caption: FormattedText::from(*voice.caption),
                note: Box::new(VoiceNote {
                    duration: voice.voice_note.duration,
                    waveform: waveform(&voice.voice_note.waveform),
                    file: File::from(*voice.voice_note.voice),
                    is_listened: voice.is_listened,
                }),
            },
            MessageContent::MessageVideoNote(note) => {
                let video = note.video_note;
                Self::VideoNote {
                    media: Box::new(Media {
                        thumbnail: video
                            .minithumbnail
                            .and_then(|thumbnail| Minithumbnail::from_td(*thumbnail)),
                        dimensions: Dimensions::new(video.length, video.length),
                        preview: still(video.thumbnail),
                        duration: video.duration,
                        file: Some(File::from(*video.video)),
                        ..Media::default()
                    }),
                    is_viewed: note.is_viewed,
                }
            }
            MessageContent::MessageSticker(sticker) => {
                let sticker = *sticker.sticker;
                Self::Sticker {
                    emoji: sticker.emoji.clone(),
                    picture: Some(Box::new(StickerPicture::from(sticker))),
                }
            }
            MessageContent::MessageAnimatedEmoji(emoji) => Self::Emoji {
                sticker: StickerPicture::from_animated_emoji(*emoji.animated_emoji).map(Box::new),
                emoji: emoji.emoji,
            },
            // shortcut: a slot machine shows its emoji, not its five layered reels; with the Lottie player (M8).
            MessageContent::MessageDice(dice) => Self::Emoji {
                sticker: match dice.final_state.map(|state| *state) {
                    Some(DiceStickers::Regular(regular)) => {
                        Some(Box::new(StickerPicture::from(*regular.sticker)))
                    }
                    _ => None,
                },
                emoji: dice.emoji,
            },
            MessageContent::MessageContact(contact) => {
                let contact = contact.contact;
                Self::Contact(Box::new(SharedContact {
                    first_name: contact.first_name,
                    last_name: contact.last_name,
                    phone: Secret::new(contact.phone_number),
                    user: Some(UserId(contact.user_id)).filter(|user| user.0 != 0),
                    has_vcard: !contact.vcard.is_empty(),
                }))
            }
            MessageContent::MessageLiveLocation(live) => {
                Self::LiveLocation(Point::from(&*live.location.location))
            }
            MessageContent::MessageLocation(location) => {
                Self::Location(Point::from(&*location.location))
            }
            MessageContent::MessageVenue(venue) => Self::Venue {
                point: Point::from(&*venue.venue.location),
                title: venue.venue.title,
                address: venue.venue.address,
            },
            MessageContent::MessagePoll(poll) => Self::Poll(Box::new(Poll::from(*poll.poll))),
            MessageContent::MessageChecklist(checklist) => {
                Self::Checklist(Box::new(crate::checklist::Checklist::from(*checklist.list)))
            }
            MessageContent::MessageGame(game) => Self::Game {
                title: game.game.title,
            },
            MessageContent::MessageInvoice(invoice) => Self::Invoice {
                title: invoice.product_info.title,
            },
            MessageContent::MessageStory(_) => Self::Story,
            MessageContent::MessageCall(call) => Self::Call {
                is_video: call.is_video,
                is_missed: matches!(*call.discard_reason, CallDiscardReason::Missed(_)),
            },
            MessageContent::MessageUnsupported(_) => Self::Unsupported,
            other => Self::Service(Service::from_td(other).unwrap_or(Service::Other)),
        }
    }
}
