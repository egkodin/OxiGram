//! Opening a downloaded file with the system (`Data::LaunchWithWarning`):
//! a run that could harm the computer, or open a file type nobody knows,
//! asks first; so does a file that can load from the web and expose the
//! IP address.

use std::path::Path;
use std::rc::Rc;

use gpui::{App, Entity, Window};
use i18n::key;

use crate::boxes::{self, ConfirmArgs};
use crate::lang::tr;
use crate::layers::Layers;

/// What a file's extension says it is (`Core::NameType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NameType {
    Executable,
    Benign,
    Unknown,
}

/// tdesktop's lists (`Core::DetectNameType`); everything not executable is
/// one kind here, as only the warning needs it.
const BENIGN: &str = "afdesign ai avif bmp dng gif heic icns ico jfif jpeg jpg jpg-large jxl nef \
png png-large psd qoi raw sketch svg tga tif tiff webp \
3g2 3gp 3gpp aep avi flv h264 m4s m4v mkv mov mp4 mpeg mpg ogv srt tgs tgv vob webm wmv \
aac ac3 aif amr caf cda cue flac m4a m4b mid midi mp3 ogg opus wav wma \
pdf doc docx ppt pptx pps ppsx xls xlsx txt rtf odt ods odp csv text log tl \
tex xspf xml djvu diag ps ost kml pub epub mobi cbr cbz fb2 prc ris pem p7b \
m3u m3u8 wpd wpl htm html xhtml key \
7z arj bz2 gz rar tar xz z zip zst \
tdesktop-theme tdesktop-palette tgios-theme attheme \
c cc cpp cxx h m mm swift cs ts class java css ninja cmake patch diff plist \
gyp gitignore strings asoundrc torrent csr json xaml md keylayout sql sln xib mk \
dmg img iso vcd \
pdb eot ics ips ipa core mem pcap ovpn part pcapng dmp pkpass dat zxp crash \
file bak gbr plain dlc fon fnt otf ttc ttf gpx db rss cur tdesktop-endpoints";

#[cfg(target_os = "macos")]
const EXECUTABLE: &str = "applescript action app bin command csh osx workflow terminal url caction \
mpkg pkg scpt scptd xhtm xhtml webarchive";
#[cfg(target_os = "windows")]
const EXECUTABLE: &str = "ad ade adp ahk app application appref-ms asp aspx asx bas bat bin cab \
cdxml cer cfg cgi chi chm cmd cnt com conf cpl crt csh der diagcab dll drv eml exe fon fxp \
gadget grp hlp hpj hta htt inf ini ins inx isp isu its jar jnlp job js jse jsp key ksh lexe \
library-ms lnk local lua mad maf mag mam manifest maq mar mas mat mau mav maw mcf mda mdb mde \
mdt mdw mdz mht mhtml mjs mmc mof msc msg msh msh1 msh2 msh1xml msh2xml mshxml msi msp mst ops \
osd paf pcd phar php php3 php4 php5 php7 phps php-s pht phtml pif pl plg pm pod prf prg ps1 \
ps2 ps1xml ps2xml psc1 psc2 psd1 psm1 pssc pst py py3 pyc pyd pyi pyo pyw pyzw pyz rb reg rgs \
scf scr sct search-ms settingcontent-ms sh shb shs slk sys swf t tmp u3p url vb vbe vbp vbs \
vbscript vdx vsmacros vsd vsdm vsdx vss vssm vssx vst vstm vstx vsw vsx vtx website wlua ws \
wsc wsf wsh xbap xll xlsb xlsm xnk xs";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const EXECUTABLE: &str = "bin csh deb desktop ksh out pet pkg pup rpm run sh shar slp zsh";

/// Files that may load remote content when opened (`IsIpRevealingPath`).
#[cfg(target_os = "macos")]
const IP_REVEALING: &str = "htm html svg m4v m3u m3u8 xhtml xml kml kmz xspf \
docx dotx docm dotm xlsx xltx xlsm xltm xlsb pptx ppsx potx pptm ppsm potm";
#[cfg(target_os = "windows")]
const IP_REVEALING: &str = "htm html svg m4v m3u m3u8 xhtml xml kml kmz xspf wpl";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const IP_REVEALING: &str = "htm html svg m4v m3u m3u8 xhtml xml kml kmz xspf";

/// The lowercased extension after the last dot of the file name.
fn extension(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let (_, extension) = name.rsplit_once('.')?;
    (!extension.is_empty()).then(|| extension.to_lowercase())
}

fn listed(list: &str, extension: &str) -> bool {
    list.split_whitespace().any(|item| item == extension)
}

fn name_type(extension: &str) -> NameType {
    if listed(EXECUTABLE, extension) {
        NameType::Executable
    } else if listed(BENIGN, extension) {
        NameType::Benign
    } else {
        NameType::Unknown
    }
}

/// How opening a file goes.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Launch {
    Open,
    /// No extension: the system could run a same-named executable, so the
    /// file only shows in its folder (`UnsafeShowOpenWith`).
    Reveal,
    Warn {
        kind: Warning,
        extension: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Warning {
    Executable,
    Unknown,
    IpReveal,
}

/// Whether a chat's file opens straight away (`LauncherWouldWarn`); files
/// from verified chats never warn.
// shortcut: no "Don't ask again" per extension, which needs the trusted types setting (M7).
fn decide(path: &Path, is_verified_chat: bool) -> Launch {
    let Some(extension) = extension(path) else {
        return Launch::Reveal;
    };
    if is_verified_chat {
        return Launch::Open;
    }
    let kind = match name_type(&extension) {
        NameType::Executable => Warning::Executable,
        _ if listed(IP_REVEALING, &extension) => Warning::IpReveal,
        NameType::Unknown => Warning::Unknown,
        NameType::Benign => return Launch::Open,
    };
    Launch::Warn { kind, extension }
}

/// Opens `path` with its default app, asking first when `decide` says so.
pub(crate) fn open(
    path: &Path,
    is_verified_chat: bool,
    layers: &Entity<Layers>,
    window: &mut Window,
    cx: &mut App,
) {
    let (kind, extension) = match decide(path, is_verified_chat) {
        Launch::Open => return cx.open_with_system(path),
        Launch::Reveal => return cx.reveal_path(path),
        Launch::Warn { kind, extension } => (kind, extension),
    };
    let text = match kind {
        Warning::IpReveal => tr(cx, key::lng_launch_svg_warning).to_string(),
        Warning::Executable | Warning::Unknown => {
            let template = if kind == Warning::Executable {
                key::lng_launch_exe_warning
            } else {
                key::lng_launch_other_warning
            };
            i18n::fill(
                &tr(cx, template),
                &[("extension", &format!("**.{extension}**"))],
            )
        }
    };
    let mut args = ConfirmArgs::new(ui::Markup::rich(&text));
    args.confirm_text = Some(tr(
        cx,
        if kind == Warning::Executable {
            key::lng_launch_exe_sure
        } else {
            key::lng_launch_other_sure
        },
    ));
    args.cancel_text = Some(tr(cx, key::lng_cancel));
    let path = path.to_path_buf();
    args.confirmed = Some(Rc::new(move |_, cx| cx.open_with_system(&path)));
    boxes::confirm(layers, args, window, cx);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch(name: &str) -> Launch {
        decide(Path::new(name), false)
    }

    fn warns(name: &str, kind: Warning) -> bool {
        matches!(launch(name), Launch::Warn { kind: found, .. } if found == kind)
    }

    #[test]
    fn opens_known_benign_files() {
        assert_eq!(launch("/tmp/report.PDF"), Launch::Open);
        assert_eq!(launch("photo.jpeg"), Launch::Open);
        assert_eq!(launch("archive.tar.gz"), Launch::Open);
    }

    #[test]
    fn asks_before_unknown_and_ip_revealing_files() {
        assert!(warns("strange.qqq", Warning::Unknown));
        assert!(warns("page.html", Warning::IpReveal));
        assert!(warns("drawing.svg", Warning::IpReveal));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn asks_before_running_mac_executables() {
        assert!(warns("Installer.pkg", Warning::Executable));
        assert!(warns("tool.command", Warning::Executable));
        assert!(warns("Script.scpt", Warning::Executable));
    }

    #[test]
    fn only_reveals_files_without_an_extension() {
        assert_eq!(launch("/tmp/run-me"), Launch::Reveal);
        assert_eq!(launch("/tmp/trailing."), Launch::Reveal);
        assert_eq!(decide(Path::new("run-me"), true), Launch::Reveal);
    }

    #[test]
    fn trusts_verified_chats() {
        assert_eq!(decide(Path::new("Installer.pkg"), true), Launch::Open);
    }
}
