//! Prepared locale-sensitive numeric date-plus-time presentation.
use crate::date_time_calendar::{self, Calendar};
use fixed_decimal::Decimal;
use icu_decimal::{
    DecimalFormatter,
    options::{DecimalFormatterOptions, GroupingStrategy},
};
use icu_locale_core::Locale;
use icu_locale_fallback::{LocaleFallbackConfig, LocaleFallbacker};
use std::{collections::BTreeMap, io};

/// A fixed-profile formatter captured from supplied locale observations.
///
/// The caller owns the clock and time zone. This formatter consumes Gregorian
/// local wall fields and uses committed recipes for the user's default calendar,
/// hour cycle, digits, punctuation and day periods. It retains no environment reader.
pub struct LocaleDateTimeFormat {
    /// Parsed segments and calendar preferences.
    recipe: Recipe,
    /// Native digits, with grouping disabled.
    decimal: DecimalFormatter,
}
impl LocaleDateTimeFormat {
    /// Captures the first present `LC_ALL`, `LC_MESSAGES`, or `LANG` observation.
    /// Missing observations, `C`, and `POSIX` select en-US. Unsupported languages
    /// use root patterns, retaining the original territory's calendar and hour cycle.
    ///
    /// # Errors
    /// Returns an error for malformed committed data or unavailable decimal data.
    pub fn new(environment: &dyn Fn(&str) -> Option<String>) -> io::Result<Self> {
        let locale = crate::locale::select(environment);
        let recipe = select_recipe(&locale)?;
        let digits: icu_locale_core::Locale = format!("und-u-nu-{}", recipe.numbers)
            .parse()
            .map_err(io::Error::other)?;
        let mut options = DecimalFormatterOptions::default();
        options.grouping_strategy = Some(GroupingStrategy::Never);
        let decimal =
            DecimalFormatter::try_new(digits.into(), options).map_err(io::Error::other)?;
        Ok(Self { recipe, decimal })
    }

    /// Formats Gregorian `[year, month, day]` and local `[hour, minute, second]`.
    /// Midnight is hour zero; fractional seconds and zone names are not displayed.
    /// The default Gregorian, Buddhist and Persian calendars are supported.
    ///
    /// # Errors
    /// Returns an error for invalid Gregorian dates or time fields.
    pub fn format(&self, date: [i32; 3], time: [u8; 3]) -> io::Result<String> {
        if time[0] > 23 || time[1] > 59 || time[2] > 59 {
            return Err(io::Error::other("invalid local wall time"));
        }
        let date = date_time_calendar::convert(date, self.recipe.calendar)?;
        let mut output = String::new();
        for &segment in &self.recipe.segments {
            match segment {
                Segment::Period => output.push_str(self.recipe.periods[usize::from(time[0] >= 12)]),
                Segment::Era => output.push_str(self.recipe.era),
                Segment::Literal(text) => output.push_str(text),
                Segment::Field { code, width } => {
                    let mut decimal = Decimal::from(self.field(code, date, time));
                    decimal.pad_start(width);
                    output.push_str(&self.decimal.format_to_string(&decimal));
                }
            }
        }
        Ok(output)
    }

    /// Retrieves a parsed recipe's numeric field.
    fn field(&self, code: Field, date: [i64; 3], time: [u8; 3]) -> i64 {
        let hour = if self.recipe.twelve_hour {
            (time[0] + 11) % 12 + 1
        } else {
            time[0]
        };
        match code {
            Field::Year => date[0],
            Field::Month => date[1],
            Field::Day => date[2],
            Field::Hour => i64::from(hour),
            Field::Minute => i64::from(time[1]),
            Field::Second => i64::from(time[2]),
        }
    }
}

/// A numeric wall-date or clock field.
#[derive(Clone, Copy)]
enum Field {
    /// Calendar year.
    Year,
    /// Calendar month.
    Month,
    /// Day of the month.
    Day,
    /// Hour in the selected cycle.
    Hour,
    /// Minute of the hour.
    Minute,
    /// Second of the minute.
    Second,
}

/// One parsed recipe operation.
#[derive(Clone, Copy)]
enum Segment {
    /// A numeric field with its minimum decimal width.
    Field {
        /// Wall-date or clock component.
        code: Field,
        /// Minimum number of displayed digits.
        width: i16,
    },
    /// The morning or afternoon label.
    Period,
    /// The calendar era label.
    Era,
    /// Committed punctuation or text, retained byte for byte.
    Literal(&'static str),
}

/// A parsed recipe whose text borrows committed data.
struct Recipe {
    /// Default calendar.
    calendar: Calendar,
    /// Decimal numbering system identifier.
    numbers: &'static str,
    /// Whether the recipe uses a twelve-hour clock.
    twelve_hour: bool,
    /// Field order and literal text, parsed once during preparation.
    segments: Vec<Segment>,
    /// Morning and afternoon labels.
    periods: [&'static str; 2],
    /// Calendar era label, when present in the recipe.
    era: &'static str,
}

/// Reports malformed committed data without a silent formatting fallback.
fn invalid() -> io::Error {
    io::Error::other("invalid committed date-time data")
}

/// Parses one field, period or era token with its trailing literal text.
fn token(segment: &'static str) -> io::Result<(char, Segment, &'static str)> {
    let (width, rest) = segment.strip_prefix('2').map_or((1, segment), |s| (2, s));
    let mut chars = rest.chars();
    let code = chars.next().ok_or_else(invalid)?;
    let field = match code {
        'y' => Field::Year,
        'M' => Field::Month,
        'd' => Field::Day,
        'H' => Field::Hour,
        'm' => Field::Minute,
        's' => Field::Second,
        'p' if width == 1 => return Ok((code, Segment::Period, chars.as_str())),
        'e' if width == 1 => return Ok((code, Segment::Era, chars.as_str())),
        _ => return Err(invalid()),
    };
    Ok((code, Segment::Field { code: field, width }, chars.as_str()))
}

/// Parses one complete recipe and all of its format tokens.
fn recipe(line: &'static str) -> io::Result<Recipe> {
    let fields: Vec<_> = line.split('\t').collect();
    let [calendar, numbers, cycle, pattern, am, pm, era] = fields.as_slice() else {
        return Err(invalid());
    };
    let calendar = calendar.parse()?;
    if ![
        "latn", "arab", "arabext", "beng", "deva", "cakm", "olck", "tibt", "adlm", "mymr", "nkoo",
    ]
    .contains(numbers)
        || !["h12", "h23"].contains(cycle)
    {
        return Err(invalid());
    }
    let mut parts = pattern.split('%');
    let mut segments = vec![Segment::Literal(parts.next().unwrap_or_default())];
    let mut seen = String::new();
    for part in parts {
        let (code, segment, literal) = token(part)?;
        if seen.contains(code) {
            return Err(invalid());
        }
        seen.push(code);
        segments.extend([segment, Segment::Literal(literal)]);
    }
    if !"yMdHms".chars().all(|c| seen.contains(c))
        || (seen.contains('p') && (am.is_empty() || pm.is_empty()))
        || (seen.contains('e') && era.is_empty())
    {
        return Err(invalid());
    }
    Ok(Recipe {
        calendar,
        numbers,
        twelve_hour: *cycle == "h12",
        segments,
        periods: [am, pm],
        era,
    })
}

/// Loads and validates every index reference, not only the selected locale.
fn load(
    recipe_data: &'static str,
    index_data: &'static str,
) -> io::Result<(Vec<Recipe>, BTreeMap<&'static str, usize>)> {
    let recipes: Vec<_> = recipe_data.lines().map(recipe).collect::<io::Result<_>>()?;
    let mut index = BTreeMap::new();
    for line in index_data.lines() {
        let (locale, raw) = line.split_once('\t').ok_or_else(invalid)?;
        let id = raw.parse::<usize>().map_err(|_| invalid())?;
        if recipes.get(id).is_none()
            || locale.parse::<Locale>().is_err()
            || index.insert(locale, id).is_some()
        {
            return Err(invalid());
        }
    }
    for root in ["und", "und-US", "und-FR", "und-IR", "und-TH"] {
        if !index.contains_key(root) {
            return Err(invalid());
        }
    }
    Ok((recipes, index))
}

/// Selects a preference representative while retaining the supplied territory.
fn representative(
    locale: &Locale,
    calendar_data: &str,
    hour_data: &str,
) -> io::Result<Option<&'static str>> {
    let Some(region) = locale.id.region else {
        return Ok(None);
    };
    let region = region.to_string();
    let mut calendars = BTreeMap::new();
    for line in calendar_data.lines() {
        let (region, calendar) = line.split_once('\t').ok_or_else(invalid)?;
        if !["persian", "buddhist"].contains(&calendar)
            || calendars.insert(region, calendar).is_some()
        {
            return Err(invalid());
        }
    }
    match calendars.get(region.as_str()).copied() {
        Some("persian") => return Ok(Some("IR")),
        Some("buddhist") => return Ok(Some("TH")),
        _ => {}
    }
    let mut hours = BTreeMap::new();
    for line in hour_data.lines() {
        let (region, cycle) = line.split_once('\t').ok_or_else(invalid)?;
        if !["h12", "h23"].contains(&cycle) || hours.insert(region, cycle).is_some() {
            return Err(invalid());
        }
    }
    let language_region = format!("{}-{region}", locale.id.language);
    let cycle = hours
        .get(language_region.as_str())
        .or_else(|| hours.get(region.as_str()))
        .or_else(|| hours.get("001"))
        .ok_or_else(invalid)?;
    Ok(Some(if *cycle == "h12" { "US" } else { "FR" }))
}

/// Matches exact data first, then locale parents with original preferences.
fn select_recipe(locale: &Locale) -> io::Result<Recipe> {
    let (mut recipes, index) = load(
        include_str!("../data/date-time/recipes.tsv"),
        include_str!("../data/date-time/index.tsv"),
    )?;
    let mut found = index.get(locale.id.to_string().as_str()).copied();
    let representative = representative(
        locale,
        include_str!("../data/date-time/calendars.tsv"),
        include_str!("../data/date-time/hours.tsv"),
    )?;
    let fallbacker = LocaleFallbacker::new().for_config(LocaleFallbackConfig::default());
    let mut parents = fallbacker.fallback_for(locale.clone().into());
    while found.is_none() {
        let mut parent: Locale = parents.get().to_string().parse().map_err(|_| invalid())?;
        if !parents.get().is_unknown() && parent.id.region == locale.id.region {
            found = index.get(parent.id.to_string().as_str()).copied();
            if found.is_some() {
                break;
            }
        }
        // Unknown languages use world preferences; an exact `und` row remains distinct.
        let region = representative.or_else(|| parents.get().is_unknown().then_some("FR"));
        if let Some(region) = region {
            parent.id.region = Some(region.parse().map_err(|_| invalid())?);
        }
        found = index.get(parent.id.to_string().as_str()).copied();
        if parents.get().is_unknown() {
            break;
        }
        parents.step();
    }
    let id = found.ok_or_else(invalid)?;
    Ok(recipes.swap_remove(id))
}

#[cfg(test)]
mod tests;
