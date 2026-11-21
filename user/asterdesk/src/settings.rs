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
    pub quality: u8,
    pub shadows: bool,
    pub transparency: bool,
    pub blur: bool,
    pub window_animations: bool,
    pub dock_animations: bool,
    pub workspace_animations: bool,
    pub ui_animations: bool,
    pub smooth_scrolling: bool,
    pub dock_position: u8,
    pub dock_size: f32,
    pub dock_magnification: f32,
    pub dock_autohide: bool,
    pub dock_indicators: bool,
    pub frame_limit: u16,
    pub ui_scale: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            accent: 0,
            animations: 1.0,
            wallpaper: 0,
            reduce_motion: false,
            brightness: 1.0,
            quality: 1,
            shadows: true,
            transparency: true,
            blur: true,
            window_animations: true,
            dock_animations: true,
            workspace_animations: true,
            ui_animations: true,
            smooth_scrolling: true,
            dock_position: 0,
            dock_size: 1.0,
            dock_magnification: 0.45,
            dock_autohide: false,
            dock_indicators: true,
            frame_limit: 60,
            ui_scale: 1.0,
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
            quality: theme.quality.min(3),
            shadows: theme.shadows,
            transparency: theme.transparency,
            blur: theme.blur,
            window_animations: theme.window_animations,
            dock_animations: theme.dock_animations,
            workspace_animations: theme.workspace_animations,
            ui_animations: theme.ui_animations,
            smooth_scrolling: theme.smooth_scrolling,
            dock_position: theme.dock_position.min(2),
            dock_size: theme.dock_size.clamp(0.7, 1.3),
            dock_magnification: theme.dock_magnification.clamp(0.0, 1.0),
            dock_autohide: theme.dock_autohide,
            dock_indicators: theme.dock_indicators,
            frame_limit: theme.frame_limit,
            ui_scale: theme.ui_scale.clamp(1.0, 1.5),
        }
    }

    pub fn apply_to(&self, theme: &mut Theme) {
        theme.accent_index = self.accent.min(theme::ACCENTS.len() - 1);
        theme.animation_speed = self.animations.clamp(0.25, 2.0);
        theme.wallpaper = self.wallpaper.min(theme::WALLPAPER_COUNT.saturating_sub(1));
        theme.reduce_motion = self.reduce_motion;
        theme.brightness = self.brightness.clamp(0.3, 1.0);
        theme.quality = self.quality.min(3);
        theme.shadows = self.shadows;
        theme.transparency = self.transparency;
        theme.blur = self.blur;
        theme.window_animations = self.window_animations;
        theme.dock_animations = self.dock_animations;
        theme.workspace_animations = self.workspace_animations;
        theme.ui_animations = self.ui_animations;
        theme.smooth_scrolling = self.smooth_scrolling;
        theme.dock_position = self.dock_position.min(2);
        theme.dock_size = self.dock_size.clamp(0.7, 1.3);
        theme.dock_magnification = self.dock_magnification.clamp(0.0, 1.0);
        theme.dock_autohide = self.dock_autohide;
        theme.dock_indicators = self.dock_indicators;
        theme.frame_limit = self.frame_limit;
        theme.ui_scale = self.ui_scale.clamp(1.0, 1.5);
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
        ) + &format!(
            "gfx={},{},{},{},{},{:.2}\nmotion={},{},{},{},{}\ndock={},{:.2},{:.2},{},{}\n",
            self.quality.min(3),
            if self.shadows { 1 } else { 0 },
            if self.transparency { 1 } else { 0 },
            if self.blur { 1 } else { 0 },
            self.frame_limit,
            self.ui_scale.clamp(1.0, 1.5),
            if self.window_animations { 1 } else { 0 },
            if self.dock_animations { 1 } else { 0 },
            if self.workspace_animations { 1 } else { 0 },
            if self.ui_animations { 1 } else { 0 },
            if self.smooth_scrolling { 1 } else { 0 },
            self.dock_position.min(2),
            self.dock_size.clamp(0.7, 1.3),
            self.dock_magnification.clamp(0.0, 1.0),
            if self.dock_autohide { 1 } else { 0 },
            if self.dock_indicators { 1 } else { 0 },
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
            "gfx" => {
                recognised += 1;
                let parts: alloc::vec::Vec<&str> = value.split(',').collect();
                if parts.len() == 6 {
                    if let (Ok(quality), Ok(frame), Ok(scale)) = (
                        parts[0].parse::<u8>(),
                        parts[4].parse::<u16>(),
                        parts[5].parse::<f32>(),
                    ) {
                        if quality <= 3
                            && matches!(frame, 0 | 30 | 60 | 120)
                            && (1.0..=1.5).contains(&scale)
                        {
                            settings.quality = quality;
                            settings.shadows = parts[1] == "1";
                            settings.transparency = parts[2] == "1";
                            settings.blur = parts[3] == "1";
                            settings.frame_limit = frame;
                            settings.ui_scale = scale;
                        } else {
                            needs_save = true;
                        }
                    } else {
                        needs_save = true;
                    }
                } else {
                    needs_save = true;
                }
            }
            "motion" => {
                recognised += 1;
                let parts: alloc::vec::Vec<&str> = value.split(',').collect();
                if parts.len() == 5 {
                    settings.window_animations = parts[0] == "1";
                    settings.dock_animations = parts[1] == "1";
                    settings.workspace_animations = parts[2] == "1";
                    settings.ui_animations = parts[3] == "1";
                    settings.smooth_scrolling = parts[4] == "1";
                } else {
                    needs_save = true;
                }
            }
            "dock" => {
                recognised += 1;
                let parts: alloc::vec::Vec<&str> = value.split(',').collect();
                if parts.len() == 5 {
                    if let (Ok(position), Ok(size), Ok(magnification)) = (
                        parts[0].parse::<u8>(),
                        parts[1].parse::<f32>(),
                        parts[2].parse::<f32>(),
                    ) {
                        if position <= 2
                            && (0.7..=1.3).contains(&size)
                            && (0.0..=1.0).contains(&magnification)
                        {
                            settings.dock_position = position;
                            settings.dock_size = size;
                            settings.dock_magnification = magnification;
                            settings.dock_autohide = parts[3] == "1";
                            settings.dock_indicators = parts[4] == "1";
                        } else {
                            needs_save = true;
                        }
                    } else {
                        needs_save = true;
                    }
                } else {
                    needs_save = true;
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
