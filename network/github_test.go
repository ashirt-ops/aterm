package network_test

import (
	"testing"

	"github.com/google/go-github/v32/github"
	"github.com/stretchr/testify/require"
	"github.com/theparanoids/aterm/network"
)

func TestSemVerParse(t *testing.T) {
	// Test invalid versions
	require.Equal(t, network.SemVer{}, network.ParseVersion(""))
	require.Equal(t, network.SemVer{}, network.ParseVersion("2"))
	require.Equal(t, network.SemVer{}, network.ParseVersion("2.0"))
	require.Equal(t, network.SemVer{}, network.ParseVersion("VA.B.C"))

	// test valid versions
	require.Equal(t, network.NewSemVer(1, 2, 3, ""), network.ParseVersion("v1.2.3"))
	require.Equal(t, network.NewSemVer(1, 2, 3, "-extra"), network.ParseVersion("v1.2.3-extra"))
	require.Equal(t, network.NewSemVer(2, 3, 4, "-double-extra"), network.ParseVersion("v2.3.4-double-extra"))
	require.Equal(t, network.NewSemVer(9, 8, 7, ""), network.ParseVersion("9.8.7"))
}

func TestIsNewerSemVer(t *testing.T) {
	initialVersion := network.ParseVersion("v1.2.3")

	newer, diff := network.IsNewerSemVer(initialVersion, network.ParseVersion("v2.2.3"))
	require.Equal(t, 1, diff.Major)
	require.True(t, newer)

	newer, diff = network.IsNewerSemVer(initialVersion, network.ParseVersion("v1.3.3"))
	require.Equal(t, 1, diff.Minor)
	require.True(t, newer)

	newer, diff = network.IsNewerSemVer(initialVersion, network.ParseVersion("v1.2.4"))
	require.Equal(t, 1, diff.Patch)
	require.True(t, newer)

	newer, diff = network.IsNewerSemVer(initialVersion, network.ParseVersion("v1.2.3"))
	require.Equal(t, 0, diff.Major)
	require.Equal(t, 0, diff.Minor)
	require.Equal(t, 0, diff.Patch)
	require.False(t, newer)

	newer, diff = network.IsNewerSemVer(initialVersion, network.ParseVersion("v0.2.3"))
	require.Equal(t, -1, diff.Major)
	require.False(t, newer)

	newer, diff = network.IsNewerSemVer(initialVersion, network.ParseVersion(""))
	require.False(t, newer)
}

func TestCheckVersionUpdate(t *testing.T) {
	current := network.ParseVersion("v1.2.3")

	outOfDateRelease := mockRelease("v1.2.2", "Out of Date Release", "you shouldn't see this")
	firstPatch := mockRelease("v1.2.4", "First patch release", "you shouldn't see this either")
	secondPatch := mockRelease("v1.2.5", "Second patch release", "you should see this")
	minor := mockRelease("v1.3.0", "Minor Update", "you should also see this")
	major := mockRelease("v2.0.0", "Major Update", "This should really be present")

	others := []*github.RepositoryRelease{
		outOfDateRelease,
		firstPatch,
		secondPatch,
		minor,
		major,
	}

	res := network.CheckVersionUpdate(current, others)

	require.True(t, res.HasUpgrade())
	require.Equal(t, network.ParseVersion(secondPatch.GetTagName()), *(res.PatchUpgrade))
	require.Equal(t, secondPatch, res.PatchRelease)
	require.Equal(t, network.ParseVersion(minor.GetTagName()), *(res.MinorUpgrade))
	require.Equal(t, minor, res.MinorRelease)
	require.Equal(t, network.ParseVersion(major.GetTagName()), *(res.MajorUpgrade))
	require.Equal(t, major, res.MajorRelease)

	none := []*github.RepositoryRelease{}
	noneRes := network.CheckVersionUpdate(current, none)
	require.False(t, noneRes.HasUpgrade())
}

func mockRelease(tagName, name, body string) *github.RepositoryRelease {
	r := github.RepositoryRelease{
		TagName: &tagName,
		Name:    &name,
		Body:    &body,
	}

	return &r
}
