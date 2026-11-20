//! Persistent preference format.
//!
//! `version=1` is followed by validated key/value pairs. Unknown keys are
//! ignored for forward compatibility; missing or invalid values fall back to
//! defaults and mark the file for a rewrite. Corruption is reported separately
//! so the caller can preserve the bad file and write a fresh one.

use alloc::format;
use alloc::string::String;

use crate::theme::{self, Theme};

pub const VERSION: u32 = 1;
pub const PATH: &str = "/home/user/SETTINGS.CNF";
pub const BACKUP: &str = "/home/user/SETTINGS.BAK";
pub const TEMP: &str = "/home/user/SETTINGS.NEW";
pub const BAD: &str = "/home/user/SETTINGS.BAD";

#[derive(Clone, Copy)]
pub struct Settings {
    pub accent: usize,
    pub animations: f32,
    pub wallpaper: usize,
    pub reduce_motion: bool,
    pub brightness: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            accent: 0,
            animations: 1.0,
            wallpaper: 0,
            reduce_motion: false,
            brightness: 1.0,
        }
    }
}

pub struct Loaded {
    pub settings: Settings,
    pub needs_save: bool,
    pub corrupt: bool,
}

impl Settings {
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            accent: theme.accent_index.min(theme::ACCENTS.len() - 1),
            animations: theme.animation_speed.clamp(0.25, 2.0),
            wallpaper: theme
                .wallpaper
                .min(theme::WALLPAPER_COUNT.saturating_sub(1)),
            reduce_motion: theme.reduce_motion,
            brightness: theme.brightness.clamp(0.3, 1.0),
        }
    }

    pub fn apply_to(&self, theme: &mut Theme) {
        theme.accent_index = self.accent.min(theme::ACCENTS.len() - 1);
        theme.animation_speed = self.animations.clamp(0.25, 2.0);
        theme.wallpaper = self.wallpaper.min(theme::WALLPAPER_COUNT.saturating_sub(1));
        theme.reduce_motion = self.reduce_motion;
        theme.brightness = self.brightness.clamp(0.3, 1.0);
    }

    pub fn serialize(&self) -> String {
        format!(
            "version={}\naccent={}\nanimations={:.2}\nwallpaper={}\nreduce_motion={}\nbrightness={:.2}\n",
            VERSION,
            theme::ACCENTS[self.accent.min(theme::ACCENTS.len() - 1)]
                .name
                .to_lowercase(),
            self.animations.clamp(0.25, 2.0),
            self.wallpaper.min(theme::WALLPAPER_COUNT.saturating_sub(1)),
            if self.reduce_motion { 1 } else { 0 },
            self.brightness.clamp(0.3, 1.0),
        )
    }
}

pub fn parse(text: &str) -> Loaded {
    let mut settings = Settings::default();
    let mut needs_save = false;
    let mut version = None;
    let mut recognised = 0usize;

    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "version" => match value.parse::<u32>() {
                Ok(found) if found == VERSION => version = Some(found),
                _ => {
                    return Loaded {
                        settings,
                        needs_save: true,
                        corrupt: true,
                    };
                }
            },
            "accent" => {
                recognised += 1;
                let wanted = value.to_lowercase();
                match theme::ACCENTS
                    .iter()
                    .position(|accent| accent.name.eq_ignore_ascii_case(&wanted))
                {
                    Some(index) => settings.accent = index,
                    None => needs_save = true,
                }
            }
            "animations" => {
                recognised += 1;
                match value.parse::<f32>() {
                    Ok(speed) if (0.25..=2.0).contains(&speed) => settings.animations = speed,
                    _ => needs_save = true,
                }
            }
            "wallpaper" => {
                recognised += 1;
                match value.parse::<usize>() {
                    Ok(index) if index < theme::WALLPAPER_COUNT => settings.wallpaper = index,
                    _ => needs_save = true,
                }
            }
            "reduce_motion" => {
                recognised += 1;
                match value {
                    "0" => settings.reduce_motion = false,
                    "1" => settings.reduce_motion = true,
                    _ => needs_save = true,
                }
            }
            "brightness" => {
                recognised += 1;
                match value.parse::<f32>() {
                    Ok(brightness) if (0.3..=1.0).contains(&brightness) => {
                        settings.brightness = brightness
                    }
                    _ => needs_save = true,
                }
            }
            _ => {}
        }
    }

    if version != Some(VERSION) {
        // A file without a version is migrated if it has recognised keys.
        // A file with no recognised keys at all is treated as corrupt.
        needs_save = true;
    }
    let corrupt = recognised == 0 && !text.trim().is_empty() && version.is_none();

    Loaded {
        settings,
        needs_save: needs_save || corrupt,
        corrupt,
    }
}
