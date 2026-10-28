//! Points on the map in messages (`Data::LocationPoint`) and their map
//! pictures, which TDLib renders as files (`getMapThumbnailFile`).

use std::future::Future;

use futures::FutureExt as _;
use td_api::functions::GetMapThumbnailFile;

use crate::file::File;
use crate::ids::ChatId;
use crate::session::{RequestError, Session};

/// Millionths of a degree in one unit: tdesktop's six decimals.
const PER_DEGREE: f64 = 1_000_000.0;

/// A point in whole millionths of a degree, so it compares exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Point {
    latitude: i64,
    longitude: i64,
}

#[expect(clippy::cast_possible_truncation, reason = "degrees are within ±180")]
fn units(degrees: f64) -> i64 {
    (degrees * PER_DEGREE).round() as i64
}

#[expect(clippy::cast_precision_loss, reason = "millionths of ±180 fit an f64")]
fn degrees(units: i64) -> f64 {
    units as f64 / PER_DEGREE
}

impl Point {
    #[must_use]
    pub fn new(latitude: f64, longitude: f64) -> Self {
        Self {
            latitude: units(latitude),
            longitude: units(longitude),
        }
    }

    #[must_use]
    pub fn latitude(self) -> f64 {
        degrees(self.latitude)
    }

    #[must_use]
    pub fn longitude(self) -> f64 {
        degrees(self.longitude)
    }

    /// The point in Maps (`Platform::LaunchMaps` on macOS, else
    /// `LocationClickHandler::Url`).
    #[must_use]
    pub fn maps_url(self) -> String {
        let (lat, lon) = (self.latitude(), self.longitude());
        if cfg!(target_os = "macos") {
            format!("https://maps.apple.com/?q=Point&z=16&ll={lat:.6},{lon:.6}")
        } else {
            format!("https://maps.google.com/maps?q={lat:.6},{lon:.6}&ll={lat:.6},{lon:.6}&z=16")
        }
    }
}

impl From<&td_api::types::Location> for Point {
    fn from(location: &td_api::types::Location) -> Self {
        Self::new(location.latitude, location.longitude)
    }
}

/// How TDLib draws a map picture (`ComputeLocation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapSize {
    pub width: i32,
    pub height: i32,
    pub zoom: i32,
    pub scale: i32,
}

impl Session {
    /// The file of `point`'s map picture, to download like any other.
    pub fn map_thumbnail(
        &self,
        point: Point,
        size: MapSize,
        chat: ChatId,
    ) -> impl Future<Output = Result<File, RequestError>> + Send + use<> {
        self.client()
            .request(&GetMapThumbnailFile {
                location: Box::new(td_api::types::Location {
                    latitude: point.latitude(),
                    longitude: point.longitude(),
                    horizontal_accuracy: 0.0,
                }),
                zoom: size.zoom,
                width: size.width,
                height: size.height,
                scale: size.scale,
                chat_id: chat.0,
            })
            .map(|result| result.map(File::from).map_err(RequestError::from))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_keep_six_decimals() {
        let point = Point::new(55.751_244_4, 37.618_423_1);
        assert_eq!(point, Point::new(55.751_244, 37.618_423));
        assert!((point.latitude() - 55.751_244).abs() < 1e-9);
        assert!(point.maps_url().contains("55.751244,37.618423"));
    }
}
