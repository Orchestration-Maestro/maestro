//! Prepared locale-sensitive numeric date-plus-time presentation.
use crate::date_time_calendar;
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
    /// Validated pattern and calendar preferences.
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
        let mut segments = self.recipe.pattern.split('%');
        let mut output = segments.next().unwrap_or_default().to_owned();
        for segment in segments {
            let (code, width, literal) = token(segment)?;
            match code {
                'p' => output.push_str(self.recipe.periods[usize::from(time[0] >= 12)]),
                'e' => output.push_str(self.recipe.era),
                _ => {
                    let value = self.field(code, date, time)?;
                    let mut decimal = Decimal::from(value);
                    decimal.pad_start(width);
                    output.push_str(&self.decimal.format_to_string(&decimal));
                }
            }
            output.push_str(literal);
        }
        Ok(output)
    }

    /// Retrieves a validated recipe's numeric field.
    fn field(&self, code: char, date: [i64; 3], time: [u8; 3]) -> io::Result<i64> {
        let hour = if self.recipe.twelve_hour {
            (time[0] + 11) % 12 + 1
        } else {
            time[0]
        };
        match code {
            'y' => Ok(date[0]),
            'M' => Ok(date[1]),
            'd' => Ok(date[2]),
            'H' => Ok(i64::from(hour)),
            'm' => Ok(i64::from(time[1])),
            's' => Ok(i64::from(time[2])),
            _ => Err(io::Error::other("invalid date-time numeric token")),
        }
    }
}

/// A validated recipe whose fields borrow committed data.
struct Recipe {
    /// Default calendar identifier.
    calendar: &'static str,
    /// Decimal numbering system identifier.
    numbers: &'static str,
    /// Whether the recipe uses a twelve-hour clock.
    twelve_hour: bool,
    /// Percent-token field order and literal text.
    pattern: &'static str,
    /// Morning and afternoon labels.
    periods: [&'static str; 2],
    /// Calendar era label, when present in the recipe.
    era: &'static str,
}

/// Reports malformed committed data without a silent formatting fallback.
fn invalid() -> io::Error {
    io::Error::other("invalid committed date-time data")
}

/// Reads a numeric token and its optional two-digit width.
fn token(segment: &str) -> io::Result<(char, i16, &str)> {
    let (width, rest) = segment.strip_prefix('2').map_or((1, segment), |s| (2, s));
    let mut chars = rest.chars();
    let code = chars.next().ok_or_else(invalid)?;
    if !"yMdHmspe".contains(code) || (width == 2 && "pe".contains(code)) {
        return Err(invalid());
    }
    Ok((code, width, chars.as_str()))
}

/// Validates one complete recipe and all of its format tokens.
fn recipe(line: &'static str) -> io::Result<Recipe> {
    let fields: Vec<_> = line.split('\t').collect();
    let [calendar, numbers, cycle, pattern, am, pm, era] = fields.as_slice() else {
        return Err(invalid());
    };
    if !["gregory", "persian", "buddhist"].contains(calendar)
        || ![
            "latn", "arab", "arabext", "beng", "deva", "cakm", "olck", "tibt", "adlm", "mymr",
            "nkoo",
        ]
        .contains(numbers)
        || !["h12", "h23"].contains(cycle)
    {
        return Err(invalid());
    }
    let mut seen = String::new();
    for segment in pattern.split('%').skip(1) {
        let (code, _, _) = token(segment)?;
        if seen.contains(code) {
            return Err(invalid());
        }
        seen.push(code);
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
        pattern,
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
fn representative(locale: &Locale) -> io::Result<Option<&'static str>> {
    let Some(region) = locale.id.region else {
        return Ok(None);
    };
    let region = region.to_string();
    let mut calendars = BTreeMap::new();
    for line in include_str!("../data/date-time/calendars.tsv").lines() {
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
    for line in include_str!("../data/date-time/hours.tsv").lines() {
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
    let representative = representative(locale)?;
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
    if id >= recipes.len() {
        return Err(invalid());
    }
    Ok(recipes.swap_remove(id))
}

#[cfg(test)]
mod tests;
