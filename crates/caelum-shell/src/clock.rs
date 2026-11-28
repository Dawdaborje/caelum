// SPDX-License-Identifier: GPL-3.0-or-later
//! The bar clock: one timer that re-arms itself for the next minute (or second)
//! boundary, so an idle shell wakes once a minute rather than every second.
use std::time::Duration;

use caelum_ui::ClockTime;
use chrono::{Local, NaiveTime, Timelike};
use slint::{Timer, TimerMode};

use crate::app;

thread_local! {
    static TIMER: Timer = Timer::default();
}

/// Milliseconds to the next whole minute (or second); a whole period when `now`
/// is exactly on a boundary, never 0.
pub fn ms_until_next_tick(now: NaiveTime, with_seconds: bool) -> u64 {
    // nanosecond() exceeds 1e9 during a leap second; count that as the last ms.
    let ms = u64::from((now.nanosecond() / 1_000_000).min(999));
    let (period, into) = if with_seconds {
        (1_000, ms)
    } else {
        (60_000, u64::from(now.second()) * 1_000 + ms)
    };
    period - into % period
}

/// The clock's parts, as Caelestia's Time service ("hh"/"HH", lowercase am/pm).
pub fn now(twelve_hour: bool) -> ClockTime {
    let now = Local::now();
    let f = |fmt: &str| now.format(fmt).to_string().into();
    ClockTime {
        weekday: f("%a"),
        day: f("%-d"),
        hour: f(if twelve_hour { "%I" } else { "%H" }),
        minute: f("%M"),
        second: f("%S"),
        am_pm: if twelve_hour {
            f("%P")
        } else {
            Default::default()
        },
    }
}

/// Refreshes every bar's clock and re-arms the timer. Call again after a config
/// change; restarting the one timer replaces the pending tick.
pub fn tick() {
    app::on_clock();
    let wait = ms_until_next_tick(Local::now().time(), app::clock_seconds());
    TIMER.with(|t| t.start(TimerMode::SingleShot, Duration::from_millis(wait), tick));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(h: u32, m: u32, s: u32, ms: u32) -> NaiveTime {
        NaiveTime::from_hms_milli_opt(h, m, s, ms).unwrap()
    }

    #[test]
    fn next_tick_is_minute_boundary() {
        assert_eq!(ms_until_next_tick(t(12, 0, 37, 250), false), 22_750);
    }

    #[test]
    fn next_tick_is_second_boundary() {
        assert_eq!(ms_until_next_tick(t(12, 0, 37, 250), true), 750);
        // Exactly on a boundary waits a whole period, never 0.
        assert_eq!(ms_until_next_tick(t(12, 0, 0, 0), false), 60_000);
    }
}
