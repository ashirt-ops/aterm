//! GitHub release update check.
//!
//! Replaces the Go `network/github.go`. Queries a repository's GitHub releases,
//! parses their tag names as *loose* semantic versions, and reports which (if
//! any) major / minor / patch upgrades are available relative to the running
//! version.
//!
//! The network layer uses `reqwest::blocking` (the project is BLOCKING-only —
//! no tokio / async). The version parsing and upgrade classification are factored
//! out as PURE functions ([`parse_version`], [`is_newer_semver`],
//! [`classify_upgrades`]) so they can be unit-tested without ever touching the
//! network.
//!
//! # Loose semver
//!
//! A tag is parsed by an optional leading `v`/`V`, then `major.minor.patch`, then
//! an arbitrary `extra` remainder (e.g. `-rc1`, `+build.7`). Comparisons IGNORE
//! the `extra` section entirely — only the numeric major/minor/patch triple is
//! considered. This mirrors the Go implementation it replaces.

use std::fmt;
use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;

// The update check is a best-effort, non-critical background notice that runs on
// the startup path before the user's first interaction (see `app::run`). It must
// not delay startup on a slow, captive, or offline network, so its timeouts are
// kept deliberately short — failing fast and silently skipping the notice is far
// better than making the user wait. These are intentionally much tighter than the
// ASHIRT API client, whose requests are user-initiated and load-bearing.

/// Connection-establishment timeout for the update-check client.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// Overall per-request timeout so an unresponsive GitHub never delays the user's
/// first interaction. Short on purpose: this is a non-blocking-feel update notice,
/// not a critical request.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(4);

/// Errors produced while checking for updates.
#[derive(Debug, Error)]
pub enum UpdateError {
    /// The HTTP request to the GitHub releases API failed (transport error or a
    /// non-success status code).
    #[error("failed to query GitHub releases")]
    Request(#[from] reqwest::Error),
}

/// A *loose* interpretation of a Semantic Version (v2).
///
/// See <https://semver.org/spec/v2.0.0.html>. Only the core `major`/`minor`/`patch`
/// triple participates in ordering; `extra` captures any trailing remainder (such
/// as a pre-release or build suffix) and is preserved for display but ignored by
/// comparisons. Parsing the `extra` further is left to the caller.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SemVer {
    /// Major version component.
    pub major: u64,
    /// Minor version component.
    pub minor: u64,
    /// Patch version component.
    pub patch: u64,
    /// Trailing remainder after `major.minor.patch` (may be empty).
    pub extra: String,
}

impl SemVer {
    /// Constructs a [`SemVer`] from its components.
    pub fn new(major: u64, minor: u64, patch: u64, extra: impl Into<String>) -> Self {
        Self {
            major,
            minor,
            patch,
            extra: extra.into(),
        }
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "v{}.{}.{}{}",
            self.major, self.minor, self.patch, self.extra
        )
    }
}

/// An available upgrade: the parsed version plus the original release tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upgrade {
    /// Parsed version of the candidate release.
    pub version: SemVer,
    /// The original release tag name (e.g. `v1.2.3`).
    pub tag: String,
}

/// The outcome of classifying a set of releases against the current version.
///
/// Each field holds the newest available upgrade of that kind, or `None` when no
/// such upgrade exists. Mirrors Go's `UpgradeResult`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpgradeResult {
    /// Newest release with a strictly greater major version.
    pub major: Option<Upgrade>,
    /// Newest release with the same major but a strictly greater minor version.
    pub minor: Option<Upgrade>,
    /// Newest release with the same major+minor but a strictly greater patch.
    pub patch: Option<Upgrade>,
}

impl UpgradeResult {
    /// Returns `true` if any major, minor, or patch upgrade is available.
    pub fn has_upgrade(&self) -> bool {
        self.major.is_some() || self.minor.is_some() || self.patch.is_some()
    }
}

/// Parses a release tag into a [`SemVer`].
///
/// Matches an optional leading `v`/`V`, then `major.minor.patch`, then captures
/// the remainder as `extra`. Mirrors the Go regex `^[vV]?(\d+)\.(\d+)\.(\d+)(.*)`.
/// Any tag that does not match (empty, missing a component, non-numeric) yields a
/// default (all-zero) [`SemVer`], exactly like the Go `ParseVersion`.
pub fn parse_version(tag: &str) -> SemVer {
    if tag.is_empty() {
        return SemVer::default();
    }

    // `[vV]?` — strip at most one leading v/V.
    let rest = tag.strip_prefix(['v', 'V']).unwrap_or(tag);

    let parsed = (|| {
        let (major, rest) = take_number(rest)?;
        let rest = rest.strip_prefix('.')?;
        let (minor, rest) = take_number(rest)?;
        let rest = rest.strip_prefix('.')?;
        let (patch, rest) = take_number(rest)?;
        Some(SemVer {
            major,
            minor,
            patch,
            extra: rest.to_string(),
        })
    })();

    parsed.unwrap_or_default()
}

/// Consumes a leading run of ASCII digits, returning the parsed value and the
/// unconsumed remainder. Returns `None` if there is no leading digit or the run
/// overflows `u64` (mirroring Go's `strconv.Atoi` failure -> default).
fn take_number(s: &str) -> Option<(u64, &str)> {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if end == 0 {
        return None;
    }
    let value = s[..end].parse().ok()?;
    Some((value, &s[end..]))
}

/// Returns `true` if `next` is a strictly newer version than `current`, comparing
/// only the major/minor/patch triple (`extra` is ignored).
///
/// Mirrors Go's `IsNewerSemVer`: a higher major; or equal major and higher minor;
/// or equal major+minor and higher patch. This is exactly lexicographic ordering
/// of the `(major, minor, patch)` triple.
pub fn is_newer_semver(current: &SemVer, next: &SemVer) -> bool {
    (next.major, next.minor, next.patch) > (current.major, current.minor, current.patch)
}

/// Classifies a list of release tags into the newest available major / minor /
/// patch upgrade relative to `current`.
///
/// PURE function over an injected list of tags — the unit-testable core that the
/// network layer feeds. Mirrors Go's `CheckVersionUpdate`: each tag is bucketed
/// by how it differs from `current` (major bump, minor bump within the same
/// major, or patch bump within the same major+minor), and within each bucket the
/// newest version wins.
pub fn classify_upgrades(current: &SemVer, tags: &[String]) -> UpgradeResult {
    let mut result = UpgradeResult::default();

    for tag in tags {
        let parsed = parse_version(tag);

        if parsed.major > current.major {
            if result
                .major
                .as_ref()
                .is_none_or(|u| is_newer_semver(&u.version, &parsed))
            {
                result.major = Some(Upgrade {
                    version: parsed,
                    tag: tag.clone(),
                });
            }
        } else if parsed.major == current.major && parsed.minor > current.minor {
            if result
                .minor
                .as_ref()
                .is_none_or(|u| is_newer_semver(&u.version, &parsed))
            {
                result.minor = Some(Upgrade {
                    version: parsed,
                    tag: tag.clone(),
                });
            }
        } else if parsed.major == current.major
            && parsed.minor == current.minor
            && parsed.patch > current.patch
            && result
                .patch
                .as_ref()
                .is_none_or(|u| is_newer_semver(&u.version, &parsed))
        {
            result.patch = Some(Upgrade {
                version: parsed,
                tag: tag.clone(),
            });
        }
    }

    result
}

/// A single GitHub release, as returned by the releases API. Only the tag name is
/// needed for the update check.
#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
}

/// Fetches the tag names of a repository's releases from the GitHub API.
///
/// Issues a blocking `GET` to
/// `https://api.github.com/repos/{owner}/{repo}/releases`. GitHub requires a
/// `User-Agent` header, so one identifying aterm is always sent. Returns the tag
/// names in the order GitHub reports them (newest first).
pub fn fetch_release_tags(owner: &str, repo: &str) -> Result<Vec<String>, UpdateError> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases");
    let user_agent = concat!("aterm/", env!("CARGO_PKG_VERSION"));

    let client = reqwest::blocking::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()?;

    let releases: Vec<GithubRelease> = client
        .get(url)
        .header(reqwest::header::USER_AGENT, user_agent)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()?
        .error_for_status()?
        .json()?;

    Ok(releases.into_iter().map(|r| r.tag_name).collect())
}

/// Checks a GitHub repository for upgrades relative to `current`.
///
/// Fetches the recent releases and classifies them with [`classify_upgrades`].
pub fn check_version(
    owner: &str,
    repo: &str,
    current: &SemVer,
) -> Result<UpgradeResult, UpdateError> {
    let tags = fetch_release_tags(owner, repo)?;
    Ok(classify_upgrades(current, &tags))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plain_triple() {
        assert_eq!(parse_version("1.2.3"), SemVer::new(1, 2, 3, ""));
    }

    #[test]
    fn parse_with_lower_v_prefix() {
        assert_eq!(parse_version("v1.2.3"), SemVer::new(1, 2, 3, ""));
    }

    #[test]
    fn parse_with_upper_v_prefix() {
        assert_eq!(parse_version("V10.20.30"), SemVer::new(10, 20, 30, ""));
    }

    #[test]
    fn parse_keeps_extra_suffix() {
        assert_eq!(parse_version("v1.2.3-rc1"), SemVer::new(1, 2, 3, "-rc1"));
        assert_eq!(
            parse_version("2.0.0+build.7"),
            SemVer::new(2, 0, 0, "+build.7")
        );
        // A trailing fourth dotted component lands entirely in `extra`.
        assert_eq!(parse_version("v1.2.3.4"), SemVer::new(1, 2, 3, ".4"));
    }

    #[test]
    fn parse_invalid_yields_default() {
        // Empty, missing a component, non-numeric, or a doubled prefix all fall
        // back to the all-zero default — matching Go's ParseVersion.
        assert_eq!(parse_version(""), SemVer::default());
        assert_eq!(parse_version("1.2"), SemVer::default());
        assert_eq!(parse_version("not-a-version"), SemVer::default());
        assert_eq!(parse_version("v.1.2.3"), SemVer::default());
        assert_eq!(parse_version("vv1.2.3"), SemVer::default());
    }

    #[test]
    fn display_round_trips_with_v_prefix() {
        assert_eq!(SemVer::new(1, 2, 3, "").to_string(), "v1.2.3");
        assert_eq!(SemVer::new(1, 2, 3, "-rc1").to_string(), "v1.2.3-rc1");
    }

    #[test]
    fn comparison_ignores_extra() {
        // Identical numeric triple but differing extra -> not newer either way.
        let a = SemVer::new(1, 2, 3, "-rc1");
        let b = SemVer::new(1, 2, 3, "+build");
        assert!(!is_newer_semver(&a, &b));
        assert!(!is_newer_semver(&b, &a));
    }

    #[test]
    fn comparison_detects_newer() {
        let cur = SemVer::new(1, 2, 3, "");
        assert!(is_newer_semver(&cur, &SemVer::new(2, 0, 0, "")));
        assert!(is_newer_semver(&cur, &SemVer::new(1, 3, 0, "")));
        assert!(is_newer_semver(&cur, &SemVer::new(1, 2, 4, "")));
        // Extra on the newer side does not change the verdict.
        assert!(is_newer_semver(&cur, &SemVer::new(1, 2, 4, "-rc1")));
    }

    #[test]
    fn comparison_detects_older_or_equal() {
        let cur = SemVer::new(1, 2, 3, "");
        assert!(!is_newer_semver(&cur, &SemVer::new(1, 2, 3, "")));
        assert!(!is_newer_semver(&cur, &SemVer::new(0, 9, 9, "")));
        assert!(!is_newer_semver(&cur, &SemVer::new(1, 1, 9, "")));
        assert!(!is_newer_semver(&cur, &SemVer::new(1, 2, 2, "")));
    }

    fn tags(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn classify_picks_newest_in_each_bucket() {
        let current = SemVer::new(1, 2, 3, "");
        let releases = tags(&[
            "v1.2.4",  // patch
            "v1.2.7",  // newer patch
            "v1.3.0",  // minor
            "v1.5.2",  // newer minor
            "v2.0.0",  // major
            "v3.1.0",  // newer major
            "v1.2.3",  // equal — ignored
            "v1.1.9",  // older — ignored
            "garbage", // unparseable -> 0.0.0, ignored
        ]);

        let result = classify_upgrades(&current, &releases);

        assert!(result.has_upgrade());
        assert_eq!(
            result.patch.as_ref().unwrap().version,
            SemVer::new(1, 2, 7, "")
        );
        assert_eq!(result.patch.as_ref().unwrap().tag, "v1.2.7");
        assert_eq!(
            result.minor.as_ref().unwrap().version,
            SemVer::new(1, 5, 2, "")
        );
        assert_eq!(
            result.major.as_ref().unwrap().version,
            SemVer::new(3, 1, 0, "")
        );
    }

    #[test]
    fn classify_ignores_extra_when_bucketing() {
        let current = SemVer::new(1, 2, 3, "");
        // Same numeric triple as current but with extra -> NOT an upgrade.
        let result = classify_upgrades(&current, &tags(&["v1.2.3-rc9"]));
        assert!(!result.has_upgrade());
    }

    #[test]
    fn classify_empty_when_no_upgrades() {
        let current = SemVer::new(2, 0, 0, "");
        let result = classify_upgrades(&current, &tags(&["v1.9.9", "v2.0.0", "v0.1.0"]));
        assert!(!result.has_upgrade());
        assert!(result.major.is_none());
        assert!(result.minor.is_none());
        assert!(result.patch.is_none());
    }

    #[test]
    fn classify_minor_requires_same_major() {
        let current = SemVer::new(1, 0, 0, "");
        // 2.5.0 is a major bump, not a minor one, despite the high minor number.
        let result = classify_upgrades(&current, &tags(&["v2.5.0"]));
        assert!(result.minor.is_none());
        assert_eq!(
            result.major.as_ref().unwrap().version,
            SemVer::new(2, 5, 0, "")
        );
    }
}
