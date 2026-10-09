//! Operator-facing session and time selection for replay lookup.
//!
//! [`SessionSelector`] and [`SessionTime`] are typed CLI values: clap parses
//! and normalizes syntax (spelling, ranges, overflow) before execution.
//! Resolution is a pure function over command-owned [`ReplaySession`] views,
//! so matching and ambiguity never see SDK session types or transport state.
//! Nothing is guessed; missing or ambiguous matches are rejected with the
//! available sessions listed.

use anyhow::{Result, anyhow, bail};

/// A session selection for `replay search-session-time --session`.
///
/// Either a numeric iRacing session number (`session_num`) or a human-facing
/// name such as `race`, `practice`, `qualify`, or `heat`. Names are normalized
/// during parsing to a compact key (lowercase alphanumerics, with common
/// spelling aliases folded), and are later resolved against the live session
/// list by [`resolve_session`].
///
/// ```
/// # use iracing_broadcast_cli::SessionSelector;
/// # use std::str::FromStr;
/// let numeric = SessionSelector::from_str("2").unwrap();
/// assert!(numeric.is_number());
/// let name = SessionSelector::from_str("Warm-up").unwrap();
/// assert_eq!(name.key(), Some("warmup"));
/// ```
///
/// # Errors
///
/// Returns an error when the input is empty, when a numeric selector does not
/// fit a `u16` session number, or when a name has no matchable characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSelector {
    kind: SelectorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SelectorKind {
    Number(u16),
    /// `(normalized match key, trimmed original for error messages)`
    Name(String, String),
}

impl SessionSelector {
    /// Whether this selector is an explicit numeric session number.
    pub fn is_number(&self) -> bool {
        matches!(self.kind, SelectorKind::Number(_))
    }

    /// The normalized match key for name selectors, `None` for numbers.
    ///
    /// Exposed for tests and diagnostics; matching operates on this key.
    pub fn key(&self) -> Option<&str> {
        match &self.kind {
            SelectorKind::Number(_) => None,
            SelectorKind::Name(key, _) => Some(key),
        }
    }
}

impl std::str::FromStr for SessionSelector {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(
                "session selector is empty; pass a session number or a name such as race, \
                 practice, qualify, or heat"
                    .to_string(),
            );
        }
        if trimmed.bytes().all(|byte| byte.is_ascii_digit()) {
            let number = trimmed
                .parse::<u16>()
                .map_err(|_| format!("session number {trimmed} is out of range (0-65535)"))?;
            return Ok(Self {
                kind: SelectorKind::Number(number),
            });
        }
        let key = normalize_selector(trimmed);
        if key.is_empty() {
            return Err(format!(
                "session selector {trimmed:?} contains no matchable characters"
            ));
        }
        Ok(Self {
            kind: SelectorKind::Name(key, trimmed.to_string()),
        })
    }
}

/// A session time for `replay search-session-time --time`, parsed to
/// milliseconds.
///
/// Accepted forms are `SS`, `MM:SS`, and `HH:MM:SS`. Seconds in a two- or
/// three-part duration must be `0..=59`; minutes and hours are unbounded apart
/// from the result fitting a `u32` millisecond count. A lone `SS` component is
/// not capped, so `700` means 700 seconds.
///
/// ```
/// # use iracing_broadcast_cli::SessionTime;
/// # use std::str::FromStr;
/// assert_eq!(SessionTime::from_str("20").unwrap().millis(), 20_000);
/// assert_eq!(SessionTime::from_str("31:20").unwrap().millis(), 1_880_000);
/// assert_eq!(
///     SessionTime::from_str("5:31:20").unwrap().millis(),
///     19_880_000
/// );
/// ```
///
/// # Errors
///
/// Returns an error when the input is not `SS`, `MM:SS`, or `HH:MM:SS`, when
/// sub-minute components exceed 59, or when the millisecond result overflows
/// `u32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionTime(u32);

impl SessionTime {
    /// The session time in milliseconds, as the wire command expects it.
    pub fn millis(self) -> u32 {
        self.0
    }
}

impl std::str::FromStr for SessionTime {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, String> {
        parse_session_time(value)
            .map(Self)
            .map_err(|error| error.to_string())
    }
}

fn parse_session_time(value: &str) -> Result<u32> {
    let parts: Vec<&str> = value.trim().split(':').collect();
    if parts.len() > 3 {
        bail!(
            "invalid session time {value:?}; expected SS, MM:SS, or HH:MM:SS \
             (e.g. 20, 31:20, 5:31:20)"
        );
    }

    let mut numbers = Vec::with_capacity(parts.len());
    for part in &parts {
        match part.trim().parse::<u32>() {
            Ok(number) => numbers.push(number),
            Err(_) => bail!(
                "invalid session time {value:?}; expected SS, MM:SS, or HH:MM:SS \
                 (e.g. 20, 31:20, 5:31:20)"
            ),
        }
    }

    let seconds: u64 = match numbers[..] {
        [seconds] => seconds.into(),
        [minutes, seconds] if seconds < 60 => u64::from(minutes) * 60 + u64::from(seconds),
        [hours, minutes, seconds] if minutes < 60 && seconds < 60 => {
            u64::from(hours) * 3600 + u64::from(minutes) * 60 + u64::from(seconds)
        }
        _ => bail!(
            "invalid session time {value:?}; sub-minute components must be 0-59 \
             (e.g. 20, 31:20, 5:31:20)"
        ),
    };

    let millis = seconds * 1000;
    u32::try_from(millis).map_err(|_| {
        anyhow!("session time {value:?} exceeds the maximum representable session time")
    })
}

/// One session available for replay lookup, as the command layer sees it.
///
/// Applications map live session metadata into this view so resolution never
/// depends on SDK session types; fields are the wire session number plus the
/// labels operators type against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaySession {
    /// The iRacing session number sent to the wire command.
    pub number: u16,
    /// The session type label, such as `Race` or `Qualify`.
    pub session_type: String,
    /// The operator-facing session name, when published.
    pub name: Option<String>,
}

impl ReplaySession {
    /// Build the replay view from a published live session entry.
    ///
    /// # Errors
    ///
    /// Returns an error when the published session number does not fit the
    /// `u16` range the wire command expects.
    pub fn from_live_session(session: &iracing_sdk::schema::session::Session) -> Result<Self> {
        let number = u16::try_from(session.session_num).map_err(|_| {
            anyhow!(
                "published session number {} is outside the wire range 0-65535",
                session.session_num
            )
        })?;
        Ok(Self {
            number,
            session_type: session.session_type.clone(),
            name: session.session_name.clone(),
        })
    }
}

/// Resolve a session selector against the sessions available for replay.
///
/// Numeric selectors must exist in the list. Name selectors are matched on the
/// normalized form of each session's type and name: exact matches win over
/// substring matches.
///
/// # Errors
///
/// Returns an error when the selector matches no session or matches more than
/// one. Error messages list the sessions that are actually available so an
/// operator can recover with a numeric selector.
pub(crate) fn resolve_session(
    selector: &SessionSelector,
    sessions: &[ReplaySession],
) -> Result<ReplaySession> {
    match &selector.kind {
        SelectorKind::Number(wanted) => sessions
            .iter()
            .find(|session| session.number == *wanted)
            .cloned()
            .ok_or_else(|| {
                anyhow!(
                    "no session {wanted} in the current session metadata; available sessions: {}",
                    describe_sessions(sessions)
                )
            }),
        SelectorKind::Name(wanted, original) => {
            if sessions.is_empty() {
                bail!(
                    "the current session metadata contains no sessions to match {original:?} \
                     against"
                );
            }

            let matches = |predicate: &dyn Fn(&str) -> bool| -> Vec<ReplaySession> {
                sessions
                    .iter()
                    .filter(|session| {
                        session_candidates(session).any(|candidate| predicate(&candidate))
                    })
                    .cloned()
                    .collect()
            };

            let exact = matches(&|candidate: &str| candidate == wanted);
            match exact.len() {
                1 => return Ok(exact[0].clone()),
                0 => {}
                _ => return Err(ambiguous(original, &exact)),
            }

            let partial = matches(&|candidate: &str| candidate.contains(wanted));
            match partial.len() {
                1 => Ok(partial[0].clone()),
                0 => Err(no_match(original, sessions)),
                _ => Err(ambiguous(original, &partial)),
            }
        }
    }
}

/// Fold a session label to a compact match key: lowercase alphanumerics only,
/// so `Warm-up`, `warm up`, and `WARM_UP` all compare equal.
fn compact(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Apply alias folding to the compacted selector (spelling variants that
/// operators use interchangeably).
fn normalize_selector(selector: &str) -> String {
    let compacted = compact(selector);
    match compacted.as_str() {
        "qualifying" | "quals" => "qualify".to_string(),
        "practise" => "practice".to_string(),
        other => other.to_string(),
    }
}

/// Normalize candidate labels (`session_type`, then name) like selectors.
fn session_candidates(session: &ReplaySession) -> impl Iterator<Item = String> {
    let type_label = normalize_selector(&session.session_type);
    let name_label = session.name.as_deref().map(normalize_selector);
    [Some(type_label), name_label]
        .into_iter()
        .flatten()
        .filter(|label| !label.is_empty())
}

fn describe_session(session: &ReplaySession) -> String {
    match &session.name {
        Some(name) => format!(
            "{}={} (name {:?})",
            session.number, session.session_type, name
        ),
        None => format!("{}={}", session.number, session.session_type),
    }
}

fn describe_sessions(sessions: &[ReplaySession]) -> String {
    if sessions.is_empty() {
        return "<none>".to_string();
    }
    sessions
        .iter()
        .map(describe_session)
        .collect::<Vec<_>>()
        .join(", ")
}

fn ambiguous(original: &str, matched: &[ReplaySession]) -> anyhow::Error {
    let hits = matched
        .iter()
        .map(describe_session)
        .collect::<Vec<_>>()
        .join(", ");
    anyhow!(
        "session selector {original:?} is ambiguous; it matches [{hits}]; \
         re-run with the numeric session number"
    )
}

fn no_match(original: &str, sessions: &[ReplaySession]) -> anyhow::Error {
    anyhow!(
        "no session matches {original:?}; available sessions: {}",
        describe_sessions(sessions)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn session(number: u16, kind: &str, name: Option<&str>) -> ReplaySession {
        ReplaySession {
            number,
            session_type: kind.to_string(),
            name: name.map(str::to_string),
        }
    }

    fn weekend() -> Vec<ReplaySession> {
        vec![
            session(0, "Practice", Some("Practice")),
            session(1, "Qualify", Some("Qualify")),
            session(2, "Race", Some("Race")),
        ]
    }

    fn selector(value: &str) -> SessionSelector {
        value.parse().unwrap()
    }

    #[test]
    fn selectors_parse_numeric_and_name_forms() {
        assert!(selector("2").is_number());
        assert!(selector(" 2 ").is_number());
        assert_eq!(selector("Warm-up").key(), Some("warmup"));
        assert_eq!(selector("  QUALIFY  ").key(), Some("qualify"));
        assert_eq!(selector("quals").key(), Some("qualify"));
        assert_eq!(selector("qualifying").key(), Some("qualify"));
        assert_eq!(selector("practise").key(), Some("practice"));
    }

    #[test]
    fn selector_syntax_errors_are_rejected_at_parse_time() {
        for bad in ["", "   ", "--", "!!!"] {
            assert!(
                SessionSelector::from_str(bad).is_err(),
                "expected {bad:?} rejected"
            );
        }
        assert!(SessionSelector::from_str("99999").is_err());
    }

    #[test]
    fn duration_forms_convert_to_milliseconds() {
        assert_eq!(SessionTime::from_str("20").unwrap().millis(), 20_000);
        assert_eq!(SessionTime::from_str("31:20").unwrap().millis(), 1_880_000);
        assert_eq!(
            SessionTime::from_str("5:31:20").unwrap().millis(),
            19_880_000
        );
        assert_eq!(SessionTime::from_str(" 7 ").unwrap().millis(), 7_000);
        assert_eq!(SessionTime::from_str("0").unwrap().millis(), 0);
        assert_eq!(SessionTime::from_str("90:20").unwrap().millis(), 5_420_000);
        assert_eq!(
            SessionTime::from_str("23:59:59").unwrap().millis(),
            86_399_000
        );
    }

    #[test]
    fn single_seconds_component_is_not_capped() {
        assert_eq!(SessionTime::from_str("700").unwrap().millis(), 700_000);
    }

    #[test]
    fn malformed_durations_are_rejected() {
        for bad in ["", " ", "abc", "-5", "20.5", ":", "5:", ":20", "5:31:20:0"] {
            assert!(
                SessionTime::from_str(bad).is_err(),
                "expected {bad:?} to be rejected"
            );
        }
    }

    #[test]
    fn sub_minute_components_must_fit_their_range() {
        for bad in ["5:70", "1:60:15", "1:15:60", "31:59:60"] {
            assert!(
                SessionTime::from_str(bad).is_err(),
                "expected {bad:?} to be rejected"
            );
        }
    }

    #[test]
    fn overflowed_millisecond_results_are_rejected() {
        assert!(SessionTime::from_str("4294968").is_err());
        assert!(SessionTime::from_str("1300000:00").is_err());
    }

    #[test]
    fn numeric_selector_must_exist_in_the_session_list() {
        let resolved = resolve_session(&selector("2"), &weekend()).unwrap();
        assert_eq!(resolved.number, 2);
        let error = resolve_session(&selector("5"), &weekend()).unwrap_err();
        assert!(error.to_string().contains("no session 5"));
        assert!(error.to_string().contains("0=Practice"));
    }

    #[test]
    fn names_match_types_case_and_punctuation_insensitively() {
        let resolved = resolve_session(&selector("raCe"), &weekend()).unwrap();
        assert_eq!(resolved.number, 2);
        let resolved =
            resolve_session(&selector("warm up"), &[session(3, "Warmup", None)]).unwrap();
        assert_eq!(resolved.number, 3);
    }

    #[test]
    fn names_match_published_session_names_too() {
        let sessions = vec![
            session(0, "Test", Some("Morning Warmup")),
            session(1, "Race", Some("Feature Race")),
        ];
        assert_eq!(
            resolve_session(&selector("morning warmup"), &sessions)
                .unwrap()
                .number,
            0
        );
    }

    #[test]
    fn aliases_match_session_types_and_names() {
        for (label, wanted) in [
            ("QUALS", "qualify"),
            ("Qualifying", "quals"),
            ("Prac-tise", "practice"),
        ] {
            for candidate in [session(3, label, None), session(3, "Other", Some(label))] {
                assert_eq!(
                    resolve_session(&selector(wanted), &[candidate])
                        .unwrap()
                        .number,
                    3
                );
            }
        }
    }

    #[test]
    fn alias_equivalent_sessions_are_ambiguous() {
        let sessions = vec![
            session(0, "Qualify", None),
            session(1, "Qualifying", None),
            session(2, "Other", Some("Quals")),
        ];
        let error = resolve_session(&selector("qualify"), &sessions)
            .unwrap_err()
            .to_string();
        assert!(error.contains("ambiguous"));
        assert!(error.contains("0=Qualify"));
        assert!(error.contains("1=Qualifying"));
        assert!(error.contains("2=Other"));
    }

    #[test]
    fn exact_matches_win_over_partial_matches() {
        let sessions = vec![
            session(0, "Qualify", Some("Fast Qualify")),
            session(1, "Race", Some("Sprint Race")),
            session(2, "Test", Some("Race Weekend")),
        ];
        // One exact hit (session 1's type) and two partial hits
        // ("Sprint Race", "Race Weekend"): the exact match wins.
        assert_eq!(
            resolve_session(&selector("race"), &sessions)
                .unwrap()
                .number,
            1
        );
    }

    #[test]
    fn exact_and_partial_ambiguity_are_both_rejected() {
        let exact_ambiguity = vec![session(0, "Race", None), session(1, "Other", Some("Race"))];
        let error = resolve_session(&selector("race"), &exact_ambiguity).unwrap_err();
        assert!(error.to_string().contains("ambiguous"));
        let partial_ambiguity = vec![
            session(2, "Test", Some("Race Weekend")),
            session(3, "Open", Some("Sprint Race")),
        ];
        assert!(resolve_session(&selector("race"), &partial_ambiguity).is_err());
    }

    #[test]
    fn unique_partial_matches_resolve() {
        let sessions = vec![
            session(0, "Practice", Some("Open Practice")),
            session(1, "Race", None),
        ];
        assert_eq!(
            resolve_session(&selector("open"), &sessions)
                .unwrap()
                .number,
            0
        );
    }

    #[test]
    fn ambiguous_selectors_are_rejected_with_the_hit_list() {
        let error = resolve_session(&selector("r"), &weekend()).unwrap_err();
        assert!(error.to_string().contains("ambiguous"));
        assert!(error.to_string().contains("0=Practice"));
    }

    #[test]
    fn unmatched_selectors_list_available_sessions() {
        let error = resolve_session(&selector("heat"), &weekend()).unwrap_err();
        assert!(error.to_string().contains("no session matches \"heat\""));
        assert!(error.to_string().contains("2=Race"));
    }

    #[test]
    fn name_selectors_against_an_empty_list_are_rejected() {
        let error = resolve_session(&selector("race"), &[]).unwrap_err();
        assert!(error.to_string().contains("no sessions"));
    }
}
