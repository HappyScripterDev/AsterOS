//! Frame scheduling, dirty-region tracking and graphics telemetry.
//!
//! The compositor is software-only. This module does not pretend otherwise:
//! quality levels change real work (shadow size, blur, effects, redraw
//! coalescing), and every metric is measured from the render loop.

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::gfx::Rect;

pub const MAX_DIRTY_REGIONS: usize = 8;
pub const DIRTY_MERGE_GAP: i32 = 6;

/// Logical compositor layers, back to front. The desktop draws in this order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    Wallpaper,
    Windows,
    Dock,
    TopBar,
    Overlays,
    Cursor,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum QualityMode {
    Quality,
    Balanced,
    Performance,
    Adaptive,
}

impl QualityMode {
    pub fn code(self) -> u8 {
        match self {
            QualityMode::Quality => 0,
            QualityMode::Balanced => 1,
            QualityMode::Performance => 2,
            QualityMode::Adaptive => 3,
        }
    }

    pub fn from_code(code: u8) -> Self {
        match code {
            1 => QualityMode::Balanced,
            2 => QualityMode::Performance,
            3 => QualityMode::Adaptive,
            _ => QualityMode::Quality,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            QualityMode::Quality => "Quality",
            QualityMode::Balanced => "Balanced",
            QualityMode::Performance => "Performance",
            QualityMode::Adaptive => "Adaptive",
        }
    }

    pub fn shadow_blur(self) -> i32 {
        match self {
            QualityMode::Quality => 28,
            QualityMode::Balanced => 22,
            QualityMode::Performance => 14,
            QualityMode::Adaptive => 22,
        }
    }

    pub fn shadow_margin(self) -> i32 {
        match self {
            QualityMode::Quality => 28,
            QualityMode::Balanced => 22,
            QualityMode::Performance => 16,
            QualityMode::Adaptive => 22,
        }
    }

    pub fn blur_radius(self) -> i32 {
        match self {
            QualityMode::Quality => 4,
            QualityMode::Balanced => 3,
            QualityMode::Performance => 2,
            QualityMode::Adaptive => 3,
        }
    }

    pub fn max_dirty_regions(self) -> usize {
        match self {
            QualityMode::Quality => MAX_DIRTY_REGIONS,
            QualityMode::Balanced => 6,
            QualityMode::Performance => 4,
            QualityMode::Adaptive => 6,
        }
    }
}

/// Dirty rectangles for one frame, stored in a fixed array. Regions closer
/// than a few pixels are merged so a moving window does not create a trail of
/// tiny rectangles.
pub struct DirtyRegions {
    width: i32,
    height: i32,
    rects: [Rect; MAX_DIRTY_REGIONS],
    count: usize,
    full: bool,
    max_regions: usize,
}

impl DirtyRegions {
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            rects: [Rect::new(0, 0, 0, 0); MAX_DIRTY_REGIONS],
            count: 0,
            full: false,
            max_regions: MAX_DIRTY_REGIONS,
        }
    }

    pub fn set_quality(&mut self, quality: QualityMode) {
        self.max_regions = quality.max_dirty_regions().min(MAX_DIRTY_REGIONS);
    }

    pub fn clear(&mut self) {
        self.count = 0;
        self.full = false;
    }

    pub fn add_all(&mut self) {
        self.full = true;
        self.count = 0;
    }

    pub fn add(&mut self, rect: Rect) {
        if self.full {
            return;
        }
        let rect = rect.intersect(Rect::new(0, 0, self.width, self.height));
        if rect.is_empty() {
            return;
        }
        if rect.area() as i64 * 100 >= self.width as i64 * self.height as i64 * 62 {
            self.full = true;
            return;
        }

        let mut candidate = rect;
        let mut index = 0;
        while index < self.count {
            let existing = self.rects[index];
            if intersects_with_gap(existing, candidate, DIRTY_MERGE_GAP) {
                candidate = existing.union(candidate);
                self.rects[index] = self.rects[self.count - 1];
                self.count -= 1;
                index = 0;
            } else {
                index += 1;
            }
        }

        if candidate.area() as i64 * 100 >= self.width as i64 * self.height as i64 * 62 {
            self.full = true;
            return;
        }

        if self.count < self.max_regions.min(MAX_DIRTY_REGIONS) {
            self.rects[self.count] = candidate;
            self.count += 1;
        } else {
            // Coalesce the two smallest regions instead of growing without
            // bound. This keeps a burst of cursor/dock updates bounded.
            let mut first = 0usize;
            let mut second = 1usize;
            let mut best = self.rects[0].area() as i64 + self.rects[1].area() as i64;
            for a in 0..self.count {
                for b in (a + 1)..self.count {
                    let score = self.rects[a].area() as i64 + self.rects[b].area() as i64;
                    if score < best {
                        best = score;
                        first = a;
                        second = b;
                    }
                }
            }
            let merged = self.rects[first].union(self.rects[second]).union(candidate);
            self.rects[first] = merged;
            self.rects[second] = self.rects[self.count - 1];
            self.count -= 1;
            if merged.area() as i64 * 100 >= self.width as i64 * self.height as i64 * 62 {
                self.full = true;
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        !self.full && self.count == 0
    }

    pub fn is_full(&self) -> bool {
        self.full
    }

    pub fn regions(&self) -> &[Rect] {
        &self.rects[..self.count]
    }

    pub fn dirty_pixels(&self) -> i64 {
        if self.full {
            return self.width as i64 * self.height as i64;
        }
        self.rects[..self.count]
            .iter()
            .map(|rect| rect.area() as i64)
            .sum()
    }

    pub fn count(&self) -> usize {
        if self.full {
            1
        } else {
            self.count
        }
    }
}

fn intersects_with_gap(a: Rect, b: Rect, gap: i32) -> bool {
    !a.intersect(b).is_empty()
        || !a.inset(-gap).intersect(b).is_empty()
        || !b.inset(-gap).intersect(a).is_empty()
}

/// Measured frame timings. All values are milliseconds from the kernel uptime
/// clock; nothing is synthesised.
#[derive(Clone, Copy, Default)]
pub struct FrameStats {
    pub frames: u64,
    pub skipped_frames: u64,
    pub last_frame_ms: u64,
    pub last_input_ms: u64,
    pub last_draw_ms: u64,
    pub last_present_ms: u64,
    pub average_frame_ms: f32,
    pub worst_frame_ms: u64,
    pub dirty_regions: u32,
    pub dirty_pixels: u64,
    pub full_redraws: u64,
    pub animations: u32,
    pub active_windows: u32,
    pub rendered_frames: u64,
}

impl FrameStats {
    pub fn record(
        &mut self,
        frame_ms: u64,
        input_ms: u64,
        draw_ms: u64,
        present_ms: u64,
        dirty: &DirtyRegions,
        animations: u32,
        active_windows: u32,
    ) {
        self.frames += 1;
        self.rendered_frames += 1;
        self.last_frame_ms = frame_ms;
        self.last_input_ms = input_ms;
        self.last_draw_ms = draw_ms;
        self.last_present_ms = present_ms;
        self.worst_frame_ms = self.worst_frame_ms.max(frame_ms);
        // Exponential average; integer-ish and stable across long sessions.
        self.average_frame_ms = if self.frames == 1 {
            frame_ms as f32
        } else {
            self.average_frame_ms * 0.95 + frame_ms as f32 * 0.05
        };
        self.dirty_regions = dirty.count() as u32;
        self.dirty_pixels = dirty.dirty_pixels().max(0) as u64;
        if dirty.is_full() {
            self.full_redraws += 1;
        }
        self.animations = animations;
        self.active_windows = active_windows;
    }

    pub fn fps(&self) -> f32 {
        if self.average_frame_ms <= 0.0 {
            0.0
        } else {
            1000.0 / self.average_frame_ms
        }
    }
}

/// Target frame pacing. A missed deadline is never "corrected" by sleeping
/// longer than the budget; the animation clock is absolute time.
pub struct FramePacer {
    target_fps: u32,
    last_start: u64,
}

impl FramePacer {
    pub fn new(target_fps: u32) -> Self {
        Self {
            target_fps: target_fps.clamp(15, 240),
            last_start: 0,
        }
    }

    pub fn set_target(&mut self, target_fps: u32) {
        self.target_fps = target_fps.clamp(15, 240);
    }

    pub fn target(&self) -> u32 {
        self.target_fps
    }

    pub fn budget_ms(&self) -> u64 {
        ((1000 / self.target_fps.max(1)).max(1)) as u64
    }

    pub fn begin(&mut self, now: u64) -> u64 {
        let delta = if self.last_start == 0 {
            self.budget_ms()
        } else {
            now.saturating_sub(self.last_start).max(1)
        };
        self.last_start = now;
        delta
    }

    pub fn sleep_time(&self, elapsed_ms: u64) -> u64 {
        self.budget_ms().saturating_sub(elapsed_ms)
    }
}

/// One physical display reported by the boot framebuffer. The structure exists
/// so a future multi-monitor backend can add displays without changing the
/// compositor API; today exactly one display is real and reported.
#[derive(Clone, Copy)]
pub struct Display {
    pub id: u32,
    pub width: i32,
    pub height: i32,
    pub scale: f32,
    pub primary: bool,
}

#[derive(Clone, Copy)]
pub struct DisplaySet {
    pub displays: [Option<Display>; 4],
    pub count: usize,
}

impl DisplaySet {
    pub fn single(width: i32, height: i32) -> Self {
        Self {
            displays: [
                Some(Display {
                    id: 1,
                    width,
                    height,
                    scale: 1.0,
                    primary: true,
                }),
                None,
                None,
                None,
            ],
            count: 1,
        }
    }

    pub fn primary(&self) -> Option<Display> {
        self.displays
            .iter()
            .flatten()
            .copied()
            .find(|display| display.primary)
    }
}

static PUB_FRAMES: AtomicU64 = AtomicU64::new(0);
static PUB_RENDERED: AtomicU64 = AtomicU64::new(0);
static PUB_SKIPPED: AtomicU64 = AtomicU64::new(0);
static PUB_FRAME_MS_X10: AtomicU32 = AtomicU32::new(0);
static PUB_DRAW_MS: AtomicU32 = AtomicU32::new(0);
static PUB_PRESENT_MS: AtomicU32 = AtomicU32::new(0);
static PUB_DIRTY_REGIONS: AtomicU32 = AtomicU32::new(0);
static PUB_DIRTY_PIXELS: AtomicU64 = AtomicU64::new(0);
static PUB_FULL_REDRAWS: AtomicU64 = AtomicU64::new(0);
static PUB_ANIMATIONS: AtomicU32 = AtomicU32::new(0);
static PUB_WINDOWS: AtomicU32 = AtomicU32::new(0);

/// Lock-free snapshot of the last rendered frame, readable by Monitor.
#[derive(Clone, Copy, Default)]
pub struct EngineSnapshot {
    pub frames: u64,
    pub rendered: u64,
    pub skipped: u64,
    pub frame_ms: f32,
    pub draw_ms: u32,
    pub present_ms: u32,
    pub dirty_regions: u32,
    pub dirty_pixels: u64,
    pub full_redraws: u64,
    pub animations: u32,
    pub windows: u32,
}

pub fn publish(stats: &FrameStats) {
    PUB_FRAMES.store(stats.frames, Ordering::Relaxed);
    PUB_RENDERED.store(stats.rendered_frames, Ordering::Relaxed);
    PUB_SKIPPED.store(stats.skipped_frames, Ordering::Relaxed);
    PUB_FRAME_MS_X10.store((stats.average_frame_ms * 10.0) as u32, Ordering::Relaxed);
    PUB_DRAW_MS.store(stats.last_draw_ms as u32, Ordering::Relaxed);
    PUB_PRESENT_MS.store(stats.last_present_ms as u32, Ordering::Relaxed);
    PUB_DIRTY_REGIONS.store(stats.dirty_regions, Ordering::Relaxed);
    PUB_DIRTY_PIXELS.store(stats.dirty_pixels, Ordering::Relaxed);
    PUB_FULL_REDRAWS.store(stats.full_redraws, Ordering::Relaxed);
    PUB_ANIMATIONS.store(stats.animations, Ordering::Relaxed);
    PUB_WINDOWS.store(stats.active_windows, Ordering::Relaxed);
}

pub fn snapshot() -> EngineSnapshot {
    EngineSnapshot {
        frames: PUB_FRAMES.load(Ordering::Relaxed),
        rendered: PUB_RENDERED.load(Ordering::Relaxed),
        skipped: PUB_SKIPPED.load(Ordering::Relaxed),
        frame_ms: PUB_FRAME_MS_X10.load(Ordering::Relaxed) as f32 / 10.0,
        draw_ms: PUB_DRAW_MS.load(Ordering::Relaxed),
        present_ms: PUB_PRESENT_MS.load(Ordering::Relaxed),
        dirty_regions: PUB_DIRTY_REGIONS.load(Ordering::Relaxed),
        dirty_pixels: PUB_DIRTY_PIXELS.load(Ordering::Relaxed),
        full_redraws: PUB_FULL_REDRAWS.load(Ordering::Relaxed),
        animations: PUB_ANIMATIONS.load(Ordering::Relaxed),
        windows: PUB_WINDOWS.load(Ordering::Relaxed),
    }
}

/// A thumbnail snapshot of a real window, captured from the backbuffer.
pub struct Preview {
    pub id: u64,
    pub title: alloc::string::String,
    pub kind: crate::apps::AppKind,
    pub pixels: Vec<u32>,
    pub width: i32,
    pub height: i32,
}
