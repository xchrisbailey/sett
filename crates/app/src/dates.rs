//! Converting between sett-store's `Date` and chrono, which the app uses for
//! the local date, formatting and the date picker.

use chrono::{Datelike, NaiveDate};
use sett_store::Date;

pub(crate) fn to_chrono(date: Date) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(
        i32::from(date.year()),
        u32::from(date.month()),
        u32::from(date.day()),
    )
}

/// `None` for a year sett-store can't hold.
pub(crate) fn from_chrono(date: impl Datelike) -> Option<Date> {
    Date::new(
        u16::try_from(date.year()).ok()?,
        u8::try_from(date.month()).ok()?,
        u8::try_from(date.day()).ok()?,
    )
}

/// The local calendar date, which decides when a Task was done and whether
/// it is overdue.
pub(crate) fn today() -> Date {
    from_chrono(chrono::Local::now().date_naive()).expect("the local date is a valid calendar date")
}
