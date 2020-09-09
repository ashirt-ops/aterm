package network

import (
	"context"
	"fmt"
	"regexp"
	"strconv"

	"github.com/google/go-github/v32/github"
)

var latestOption = github.ListOptions{
	Page:    1,
	PerPage: 1,
}

var last30Option = github.ListOptions{
	Page:    1,
	PerPage: 30,
}

var githubClient *github.Client = github.NewClient(nil)
var semver2Regex = regexp.MustCompile(`^[vV]?(\d+)\.(\d+)\.(\d+)(.*)`)

// SemVer is a *loose* interpertation of Semantic Versioning (v2). Details are here:
// https://semver.org/spec/v2.0.0.html
// In general, this structure tries to capture the core details: a major, minor, patch, and "extra"
// section covering usage to try to find new versions. It is incumbent on the user to parse the "extra"
// if more details are needed beyond the Major/Minor/Patch versions
type SemVer struct {
	Major int
	Minor int
	Patch int
	Extra string
}

// NewSemVer constructs a new semver
func NewSemVer(ma, mi, pa int, ex string) SemVer {
	return SemVer{Major: ma, Minor: mi, Patch: pa, Extra: ex}
}

// String reconstructs a semver string 
func (s SemVer) String() string {
	return fmt.Sprintf("v%v.%v.%v%v", s.Major, s.Minor, s.Patch, s.Extra)
}

type UpgradeResult struct {
	MajorUpgrade *SemVer
	MajorRelease *github.RepositoryRelease
	MinorUpgrade *SemVer
	MinorRelease *github.RepositoryRelease
	PatchUpgrade *SemVer
	PatchRelease *github.RepositoryRelease
}

// HasUpgrade checks if some major, minor, or patch version has been set in the given UpgradeResult
func (u UpgradeResult) HasUpgrade() bool {
	return u.MajorUpgrade != nil || u.MinorUpgrade != nil || u.PatchUpgrade != nil
}

// CheckVersion retrieves the current releases, then calls CheckVersionUpdate on those releases
func CheckVersion(owner, repo string, current SemVer) (UpgradeResult, error) {
	releases, err := RecentReleases(owner, repo)
	if err != nil {
		return UpgradeResult{}, err
	}
	return CheckVersionUpdate(current, releases), nil

}

// CheckVersionUpdate iterates through the provided list of releases, and determines which, if any,
// are "upgrades" (ignoring any "extra" bits in the semver). An UpgradeResult is returned, indicating
// which kind of upgrades are available.
func CheckVersionUpdate(current SemVer, releases []*github.RepositoryRelease) UpgradeResult {
	up := UpgradeResult{}

	first := func(b bool, _ SemVer) bool { return b }

	for _, r := range releases {
		parsedTag := ParseVersion(r.GetTagName())
		if parsedTag.Major > current.Major {
			if up.MajorUpgrade == nil || first(IsNewerSemVer(*up.MajorUpgrade, parsedTag)) {
				up.MajorUpgrade = &parsedTag
				up.MajorRelease = r
			}
		} else if parsedTag.Major == current.Major && parsedTag.Minor > current.Minor {
			if up.MinorUpgrade == nil || first(IsNewerSemVer(*up.MinorUpgrade, parsedTag)) {
				up.MinorUpgrade = &parsedTag
				up.MinorRelease = r
			}
		} else if parsedTag.Major == current.Major && parsedTag.Minor == current.Minor && parsedTag.Patch > current.Patch {
			if up.PatchUpgrade == nil || first(IsNewerSemVer(*up.PatchUpgrade, parsedTag)) {
				up.PatchUpgrade = &parsedTag
				up.PatchRelease = r
			}
		}
	}

	return up
}

// IsNewerSemVer compares 2 SemVer structs. It will return true if the "next" SemVer has a higher
// major version, or an equal major version and a higher minor version, or an equal major and minor
// versions, and a higher patch version (extra is ignored)
// In addition, this also returns the difference, represented as a semver, of the comparison, which
// is helpful in determining how the "next" version is newer (or older). Each value in the difference
// is determined by (next.X - current.X)
func IsNewerSemVer(current SemVer, next SemVer) (bool, SemVer) {
	diff := SemVer{
		Major: next.Major - current.Major,
		Minor: next.Minor - current.Minor,
		Patch: next.Patch - current.Patch,
	}
	if diff.Major < 0 {
		return false, diff
	} else if diff.Minor < 0 && diff.Major == 0 {
		return false, diff
	} else if diff.Patch < 1 && diff.Minor == 0 && diff.Major == 0 {
		return false, diff
	}
	return true, diff
}

// RecentReleases returns the newest 30 releases (currently the max single page value from github)
// from the specified github repo
func RecentReleases(owner, repo string) ([]*github.RepositoryRelease, error) {
	return recentReleases(owner, repo, last30Option)
}

func recentReleases(owner, repo string, opts github.ListOptions) ([]*github.RepositoryRelease, error) {
	release, _, err := githubClient.Repositories.ListReleases(context.Background(), owner, repo, &opts)

	return release, err
}

// LatestRelease returns the newest release from the specified github repo
func LatestRelease(owner, repo string) (*github.RepositoryRelease, error) {
	release, err := recentReleases(owner, repo, latestOption)

	return release[0], err
}

// ParseVersion returns a SemVer for the given Semantic Version string provided.
// This will match the major, minor, and patch numbers, and provide the remainder as the "extra".
// the leading v is optional.
func ParseVersion(tagName string) SemVer {
	if tagName == "" {
		return SemVer{}
	}
	matches := semver2Regex.FindStringSubmatch(tagName)

	if matches == nil {
		return SemVer{}
	}

	maj, majErr := strconv.Atoi(matches[1])
	min, minErr := strconv.Atoi(matches[2])
	pat, patErr := strconv.Atoi(matches[3])
	ext := matches[4]

	if majErr != nil || minErr != nil || patErr != nil {
		return SemVer{}
	}

	return SemVer{
		Major: maj,
		Minor: min,
		Patch: pat,
		Extra: ext,
	}
}
