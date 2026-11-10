//! Lottie animations (TGS stickers, animated emoji, reactions and their
//! effects) rendered by tlottie, the renderer tdesktop links (ADR 0007): a
//! file opens once, then each frame renders at the size it shows at, on
//! one thread of their own ([`Renderer`]), as tlottie's renderers stay on
//! the thread that made them.

use std::collections::HashMap;
use std::future::Future;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use futures_channel::oneshot;

use flate2::read::GzDecoder;
use tlottie::{CPURenderer, ChannelOrder, Composition, FitzModifier, Limits, ParseOptions};

/// Telegram's TGS files unpack to well under this; more is not a sticker.
const MAX_JSON_BYTES: u64 = 8 << 20;
/// A frame larger than this on a side is refused, as no place shows one.
const MAX_SIDE: u32 = 2048;
/// tdesktop's `kMaxFrameRate`; files that claim more play at this rate.
const MAX_FRAME_RATE: f32 = 120.0;
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

#[derive(Debug, thiserror::Error)]
pub enum LottieError {
    #[error("reading the animation failed: {0}")]
    Read(#[from] std::io::Error),
    #[error("the animation is larger than {MAX_JSON_BYTES} bytes")]
    TooLarge,
    #[error("the animation is not valid Lottie: {0}")]
    Parse(String),
    #[error("a {0}x{1} frame is out of range")]
    Size(u32, u32),
    #[error("rendering a frame failed: {0}")]
    Render(String),
    #[error("the Lottie thread is gone")]
    Gone,
}

/// A skin tone for emoji animations (`fitzpatrick_type` 2 to 6).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SkinTone(pub u8);

impl SkinTone {
    fn modifier(self) -> FitzModifier {
        match self.0 {
            1 | 2 => FitzModifier::Type12,
            3 => FitzModifier::Type3,
            4 => FitzModifier::Type4,
            5 => FitzModifier::Type5,
            6 => FitzModifier::Type6,
            _ => FitzModifier::None,
        }
    }
}

/// One opened animation and its renderer.
pub struct Animation {
    renderer: CPURenderer,
    frames: u32,
    frame_rate: f32,
}

impl std::fmt::Debug for Animation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Animation")
            .field("frames", &self.frames)
            .field("frame_rate", &self.frame_rate)
            .finish_non_exhaustive()
    }
}

impl Animation {
    /// Opens a TGS (gzipped) or plain Lottie JSON file.
    ///
    /// # Errors
    /// When the file cannot be read, is or unpacks too large, or is not Lottie.
    pub fn open(path: &Path, tone: SkinTone) -> Result<Self, LottieError> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(MAX_JSON_BYTES + 1)
            .read_to_end(&mut bytes)?;
        Self::from_bytes(&bytes, tone)
    }

    /// # Errors
    /// When the bytes are or unpack too large, or are not Lottie.
    pub fn from_bytes(bytes: &[u8], tone: SkinTone) -> Result<Self, LottieError> {
        let json = if bytes.starts_with(&GZIP_MAGIC) {
            let mut json = Vec::new();
            GzDecoder::new(bytes)
                .take(MAX_JSON_BYTES + 1)
                .read_to_end(&mut json)?;
            json
        } else {
            bytes.to_vec()
        };
        if json.len() as u64 > MAX_JSON_BYTES {
            return Err(LottieError::TooLarge);
        }
        let options = ParseOptions {
            fitz_modifier: tone.modifier(),
            channel_order: ChannelOrder::Bgra,
            ..ParseOptions::default()
        };
        let composition = Composition::parse_with_options(&json, &Limits::default(), &options)
            .map_err(|error| LottieError::Parse(error.to_string()))?;
        let frames = composition.frame_count().max(1);
        let frame_rate = if composition.frame_rate.is_finite() {
            composition.frame_rate.clamp(1.0, MAX_FRAME_RATE)
        } else {
            MAX_FRAME_RATE
        };
        Ok(Self {
            renderer: CPURenderer::new(composition),
            frames,
            frame_rate,
        })
    }

    #[must_use]
    pub fn frame_count(&self) -> u32 {
        self.frames
    }

    #[must_use]
    pub fn frame_rate(&self) -> f32 {
        self.frame_rate
    }

    /// Frame `index` at `width` x `height` pixels, BGRA with straight alpha
    /// as gpui's sprites take, rows packed.
    ///
    /// # Errors
    /// When the size is out of range or tlottie fails.
    pub fn render(&mut self, index: u32, width: u32, height: u32) -> Result<Vec<u8>, LottieError> {
        if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
            return Err(LottieError::Size(width, height));
        }
        let count =
            usize::try_from(width * height).map_err(|_| LottieError::Size(width, height))?;
        let mut words = vec![0_u32; count];
        #[expect(clippy::cast_precision_loss, reason = "frame numbers are small")]
        let frame = (index % self.frames) as f32;
        self.renderer
            .render(
                frame,
                &mut words,
                width,
                height,
                tlottie::RenderOptions::default(),
            )
            .map_err(|error| LottieError::Render(error.to_string()))?;
        Ok(straight_bgra(&words))
    }
}

/// An animation the renderer thread holds open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimationId(u64);

impl AnimationId {
    /// An id no thread gave, for tests of what holds one.
    #[doc(hidden)]
    #[must_use]
    pub const fn for_tests(raw: u64) -> Self {
        Self(raw)
    }
}

/// What opening told.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Info {
    pub id: AnimationId,
    pub frames: u32,
    pub frame_rate: f32,
}

type Reply<T> = oneshot::Sender<Result<T, LottieError>>;

enum Job {
    Open {
        path: PathBuf,
        tone: SkinTone,
        reply: Reply<Info>,
    },
    Render {
        id: AnimationId,
        index: u32,
        size: (u32, u32),
        reply: Reply<Vec<u8>>,
    },
    Close(AnimationId),
}

/// The thread that renders every animation (`Lottie::FrameRenderer`);
/// clones send to the same thread, which ends when the last is dropped.
#[derive(Debug, Clone)]
pub struct Renderer {
    jobs: mpsc::Sender<Job>,
}

impl Renderer {
    /// # Errors
    /// When the thread cannot start.
    pub fn spawn() -> std::io::Result<Self> {
        let (jobs, inbox) = mpsc::channel();
        std::thread::Builder::new()
            .name("lottie".to_owned())
            .spawn(move || serve(&inbox))?;
        Ok(Self { jobs })
    }

    /// Opens a file; `Gone` when the thread ended.
    pub fn open(
        &self,
        path: PathBuf,
        tone: SkinTone,
    ) -> impl Future<Output = Result<Info, LottieError>> + Send + use<> {
        let (reply, answer) = oneshot::channel();
        // A gone thread drops the reply, which reads as `Gone`.
        let _ = self.jobs.send(Job::Open { path, tone, reply });
        flatten(answer)
    }

    pub fn render(
        &self,
        id: AnimationId,
        index: u32,
        width: u32,
        height: u32,
    ) -> impl Future<Output = Result<Vec<u8>, LottieError>> + Send + use<> {
        let (reply, answer) = oneshot::channel();
        let _ = self.jobs.send(Job::Render {
            id,
            index,
            size: (width, height),
            reply,
        });
        flatten(answer)
    }

    /// Frees an animation; later renders of it fail.
    pub fn close(&self, id: AnimationId) {
        let _ = self.jobs.send(Job::Close(id));
    }
}

async fn flatten<T>(answer: oneshot::Receiver<Result<T, LottieError>>) -> Result<T, LottieError> {
    answer.await.unwrap_or(Err(LottieError::Gone))
}

fn serve(inbox: &mpsc::Receiver<Job>) {
    let mut open: HashMap<AnimationId, Animation> = HashMap::new();
    let mut next = 0_u64;
    while let Ok(job) = inbox.recv() {
        match job {
            Job::Open { path, tone, reply } => {
                let opened = Animation::open(&path, tone).map(|animation| {
                    next += 1;
                    let id = AnimationId(next);
                    let info = Info {
                        id,
                        frames: animation.frame_count(),
                        frame_rate: animation.frame_rate(),
                    };
                    open.insert(id, animation);
                    info
                });
                let _ = reply.send(opened);
            }
            Job::Render {
                id,
                index,
                size: (width, height),
                reply,
            } => {
                let frame = match open.get_mut(&id) {
                    Some(animation) => animation.render(index, width, height),
                    None => Err(LottieError::Render("the animation is closed".to_owned())),
                };
                let _ = reply.send(frame);
            }
            Job::Close(id) => {
                open.remove(&id);
            }
        }
    }
}

/// tlottie's premultiplied `0xAARRGGBB` words as straight-alpha BGRA bytes.
fn straight_bgra(words: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(words.len() * 4);
    for word in words {
        let [blue, green, red, alpha] = word.to_le_bytes();
        let unmultiply = |channel: u8| match alpha {
            0 => 0,
            255 => channel,
            _ => u8::try_from(u16::from(channel) * 255 / u16::from(alpha)).unwrap_or(u8::MAX),
        };
        bytes.extend_from_slice(&[unmultiply(blue), unmultiply(green), unmultiply(red), alpha]);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 10x10, 30-frame composition filled red, its opacity animated from
    /// 0 to 100.
    const RED: &str = r##"{"v":"5.5.2","fr":30,"ip":0,"op":30,"w":10,"h":10,"layers":[
        {"ty":1,"ind":1,"ip":0,"op":30,"st":0,"sw":10,"sh":10,"sc":"#ff0000",
         "ks":{"o":{"a":1,"k":[{"t":0,"s":[0],"e":[100]},{"t":29,"s":[100]}]},
               "p":{"a":0,"k":[5,5,0]},"a":{"a":0,"k":[5,5,0]},
               "s":{"a":0,"k":[100,100,100]},"r":{"a":0,"k":0}}}]}"##;

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        use std::io::Write as _;
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(bytes).expect("writing to memory works");
        encoder.finish().expect("finishing in memory works")
    }

    #[test]
    fn a_tgs_opens_and_renders_straight_bgra() {
        let mut animation =
            Animation::from_bytes(&gzip(RED.as_bytes()), SkinTone::default()).expect("valid TGS");
        assert_eq!(animation.frame_count(), 30);
        assert!((animation.frame_rate() - 30.0).abs() < f32::EPSILON);
        let last = animation.render(29, 4, 4).expect("renders");
        assert_eq!(last.len(), 4 * 4 * 4);
        assert_eq!(&last[..4], &[0, 0, 255, 255], "opaque red, blue byte first");
        let first = animation.render(0, 4, 4).expect("renders");
        assert_eq!(first[3], 0, "transparent at frame 0");
    }

    #[test]
    fn hostile_input_is_an_error_not_a_panic() {
        assert!(Animation::from_bytes(b"{not json", SkinTone::default()).is_err());
        assert!(Animation::from_bytes(&gzip(b"[]"), SkinTone::default()).is_err());
        let mut animation =
            Animation::from_bytes(RED.as_bytes(), SkinTone::default()).expect("valid JSON");
        assert!(animation.render(0, 0, 4).is_err());
        assert!(animation.render(0, MAX_SIDE + 1, 4).is_err());
    }

    #[test]
    fn plain_json_over_the_cap_is_too_large() {
        let mut padded = RED.as_bytes().to_vec();
        padded.resize(usize::try_from(MAX_JSON_BYTES).expect("fits") + 1, b' ');
        let error = Animation::from_bytes(&padded, SkinTone::default()).expect_err("over the cap");
        assert!(matches!(error, LottieError::TooLarge), "{error}");
    }

    #[test]
    fn half_covered_pixels_lose_their_premultiplication() {
        let word = u32::from_le_bytes([0, 0, 64, 128]);
        assert_eq!(straight_bgra(&[word]), [0, 0, 127, 128]);
    }

    #[test]
    fn the_renderer_thread_opens_renders_and_closes() {
        let dir = std::env::temp_dir().join(format!("oxigram-lottie-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("red.tgs");
        std::fs::write(&path, gzip(RED.as_bytes())).expect("write");
        let renderer = Renderer::spawn().expect("thread starts");
        let info =
            futures::executor::block_on(renderer.open(path, SkinTone::default())).expect("opens");
        assert_eq!(info.frames, 30);
        let frame =
            futures::executor::block_on(renderer.render(info.id, 29, 2, 2)).expect("renders");
        assert_eq!(frame.len(), 16);
        renderer.close(info.id);
        let gone = futures::executor::block_on(renderer.render(info.id, 0, 2, 2));
        assert!(gone.is_err(), "a closed animation renders nothing");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
