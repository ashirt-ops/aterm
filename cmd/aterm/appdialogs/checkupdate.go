package appdialogs

import (
	"os"

	"github.com/theparanoids/aterm/fancy"
	"github.com/theparanoids/aterm/network"
)

func NotifyUpdate(currentVersion, owner, repo string) {
	currentSemVer := network.ParseVersion(currentVersion)
	if currentSemVer.String() == "v0.0.0-development" {
		printline("This appears to be a development release")
		return
	} else if currentSemVer.String() == "v0.0.0-unversioned" {
		printline("This application appears to be missing a version")
		return
	}

	res, err := network.CheckVersion(owner, repo, currentSemVer)
	if err != nil {
		printline("Unable to check for updates", err.Error())
	} else if res.HasUpgrade() {
		printline(fancy.AsBold("There is an update available."))
		if res.MajorUpgrade != nil {
			upgradeNoticeTemplate.Execute(os.Stdout, NewUpgrade("major", (*res.MajorUpgrade).String(), (*res.MajorRelease).GetHTMLURL()))
		}
		if res.MinorUpgrade != nil {
			upgradeNoticeTemplate.Execute(os.Stdout, NewUpgrade("minor", (*res.MinorUpgrade).String(), (*res.MinorRelease).GetHTMLURL()))
		}
		if res.PatchUpgrade != nil {
			upgradeNoticeTemplate.Execute(os.Stdout, NewUpgrade("patch", (*res.PatchUpgrade).String(), (*res.PatchRelease).GetHTMLURL()))
		}
	}
}
