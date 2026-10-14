use crate::theme::Theme;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub const ANIMATION_TICK_MILLIS: u128 = 180;
const CAT_POSE_TICKS: usize = 4;
const CAT_BLINK_TICKS: usize = 36;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MikuMotion {
    Idle,
    Thinking,
    Running,
    Meowing,
    Celebrate,
    Error,
}

impl MikuMotion {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Thinking => "thinking",
            Self::Running => "running",
            Self::Meowing => "meow",
            Self::Celebrate => "happy",
            Self::Error => "hiss",
        }
    }
}

// A 28 by 16 raster: pointy ears, layered cheeks, tiny eyes, whiskers,
// collar, paws, and a two-cell tail. Each ASCII cell maps to one terminal cell.
const BIG_CAT: &[&str] = &[
    "...........s....s...........",
    "..........##......##........",
    ".........####....####.......",
    "........######..######......",
    "......################......",
    "....####################....",
    "...######################...",
    "..####++####....####++####..",
    "..####..e##......##e..####..",
    "w.####.###.pp..pp.###.####.w",
    "..#####pp##########pp#####..",
    "...#######...mm...#######...",
    "....####################....",
    "...######....cc....######...",
    "..########################..",
    "..#######...####...#######tt",
];

/// Return a large, fixed-grid cat with slow, deliberately small movements.
/// The body never jumps: whiskers shift every 720 ms, the tail tip follows the
/// same four-pose loop, and a short blink occurs about every 6.5 seconds.
pub fn miku_pixel_cat(theme: &Theme, motion: MikuMotion, frame: usize) -> Vec<String> {
    let pose = (frame / CAT_POSE_TICKS) % 4;
    let blinking = frame % CAT_BLINK_TICKS < 2;
    let whiskers = match pose {
        1 => ["╲", "╱"],
        3 => ["╱", "╲"],
        _ => ["─", "─"],
    };
    let tail_tip = ["╯", "╱", "─", "╲"][pose];

    BIG_CAT
        .iter()
        .enumerate()
        .map(|(row_index, row)| {
            row.chars()
                .enumerate()
                .map(|(column, cell)| match cell {
                    '#' => theme.accent("█"),
                    '+' => theme.primary("▓"),
                    'e' if blinking => theme.primary("─"),
                    'e' if motion == MikuMotion::Error => theme.err("×"),
                    'e' => theme.primary("●"),
                    'p' => theme.warn("▚"),
                    'm' if motion == MikuMotion::Error => theme.err("ᴗ"),
                    'm' if motion == MikuMotion::Meowing => theme.primary("ᴏ"),
                    'm' => theme.primary("ᴗ"),
                    'c' => theme.ok("▓"),
                    's' => theme.warn("✦"),
                    'w' => theme.dim(if column == 0 {
                        whiskers[0]
                    } else {
                        whiskers[1]
                    }),
                    't' if row_index == 15 && column == 26 => theme.warn("╰"),
                    't' => theme.warn(tail_tip),
                    _ => " ".to_string(),
                })
                .collect()
        })
        .collect()
}

/// Stable one-line expressions keep the companion calm while work progresses.
pub fn miku_expression(motion: MikuMotion, _frame: usize) -> &'static str {
    match motion {
        MikuMotion::Idle => "= ^.^ =",
        MikuMotion::Thinking => "= ^.^ = ·",
        MikuMotion::Running => "= ^.^ =",
        MikuMotion::Meowing => "= ^o^ =",
        MikuMotion::Celebrate => "= ^▽^ =",
        MikuMotion::Error => "= ;_; =",
    }
}

/// Convert elapsed monotonic time to stable 180 ms animation ticks.
pub fn animation_frame_for_elapsed(elapsed: Duration) -> usize {
    (elapsed.as_millis() / ANIMATION_TICK_MILLIS).min(usize::MAX as u128) as usize
}

/// Sample one process-wide monotonic clock so fast typing cannot speed up motion.
pub fn animation_frame_now() -> usize {
    static START: OnceLock<Instant> = OnceLock::new();
    animation_frame_for_elapsed(START.get_or_init(Instant::now).elapsed())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::visible_width;
    use crate::theme::theme_by_name;

    #[test]
    fn every_pose_uses_the_same_large_detailed_block_cat() {
        let theme = theme_by_name("no-color");
        for motion in [
            MikuMotion::Idle,
            MikuMotion::Thinking,
            MikuMotion::Running,
            MikuMotion::Meowing,
            MikuMotion::Celebrate,
            MikuMotion::Error,
        ] {
            let sprite = miku_pixel_cat(&theme, motion, 2);
            let expected_eye = if motion == MikuMotion::Error {
                "×"
            } else {
                "●"
            };
            assert_eq!(sprite.len(), 16, "{motion:?}");
            assert!(sprite.iter().all(|row| visible_width(row) == 28));
            assert!(sprite.iter().any(|row| row.contains(expected_eye)));
            assert!(sprite.iter().any(|row| row.contains('▚')));
            assert!(sprite
                .iter()
                .any(|row| row.contains('ᴗ') || row.contains('ᴏ') || row.contains('﹏')));
            assert!(sprite.iter().any(|row| row.contains('─')));
            assert!(sprite.iter().any(|row| row.contains('╰')));
        }
    }

    #[test]
    fn cat_motion_is_slow_fixed_footprint_and_repeats_predictably() {
        let theme = theme_by_name("no-color");
        let at_start = miku_pixel_cat(&theme, MikuMotion::Idle, 2);
        let before_pose_change = miku_pixel_cat(&theme, MikuMotion::Idle, 3);
        let first_pose = miku_pixel_cat(&theme, MikuMotion::Idle, 4);
        let same_pose_later = miku_pixel_cat(&theme, MikuMotion::Idle, 20);
        let next_pose = miku_pixel_cat(&theme, MikuMotion::Idle, 8);

        assert_eq!(
            at_start, before_pose_change,
            "redraws within 720 ms do not advance the cat"
        );
        assert_ne!(
            at_start, first_pose,
            "the first tail/whisker pose is visible after 720 ms"
        );
        assert_eq!(
            first_pose, same_pose_later,
            "the pose loop is deterministic"
        );
        assert_ne!(
            first_pose, next_pose,
            "the next gentle pose is visible after another 720 ms"
        );
        assert!(at_start
            .iter()
            .zip(first_pose.iter())
            .all(|(first, next)| visible_width(first) == visible_width(next)));
    }

    #[test]
    fn blink_is_brief_and_expression_stays_calm() {
        let theme = theme_by_name("no-color");
        let blink = miku_pixel_cat(&theme, MikuMotion::Idle, 36);
        let open = miku_pixel_cat(&theme, MikuMotion::Idle, 38);
        assert_ne!(blink[8], open[8]);
        assert_eq!(
            miku_expression(MikuMotion::Idle, 0),
            miku_expression(MikuMotion::Idle, 10_000)
        );
    }

    #[test]
    fn monotonic_frame_clock_uses_fixed_180_ms_ticks() {
        assert_eq!(animation_frame_for_elapsed(Duration::from_millis(0)), 0);
        assert_eq!(animation_frame_for_elapsed(Duration::from_millis(179)), 0);
        assert_eq!(animation_frame_for_elapsed(Duration::from_millis(180)), 1);
        assert_eq!(animation_frame_for_elapsed(Duration::from_millis(719)), 3);
        assert_eq!(animation_frame_for_elapsed(Duration::from_millis(720)), 4);
    }
}
