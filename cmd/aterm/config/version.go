package config

import (
	"regexp"
	"runtime"
	"time"

	"github.com/jrozner/go-info"
)

var codeRepoRaw string
var codeRepo string
var codeOwner string

// UnknownCodeRepo is the value represented by CodeRepo() when no repo is specified
const UnknownCodeRepo = "???"

// UnknownCodeOwner is the value represented by CodeOwner() when no repo is specified
const UnknownCodeOwner = "???"

var parsedVersionData *info.Data
var tagRegex = regexp.MustCompile(`.*?(?:tags/)?v(.*)`)
var ownerRepoRegex = regexp.MustCompile(`^([^/]+)/(.*)`)

func getData() *info.Data {
	if parsedVersionData == nil {
		var err error
		parsedVersionData, err = info.Values()
		if err != nil {
			parsedVersionData = &info.Data{
				Version:    "v0.0.0-unversioned",
				CommitHash: "Unknown",
				Runtime:    runtime.Version(),
				BuildDate:  time.Time{},
			}
		}
		matches := tagRegex.FindStringSubmatch(parsedVersionData.Version)
		if matches != nil {
			parsedVersionData.Version = matches[1]
		}
	}
	return parsedVersionData
}

// Version extracts the tagged version from the build flags
func Version() string {
	return getData().Version
}

// CommitHash extracts the commit hash from the build flags
func CommitHash() string {
	return getData().CommitHash
}

// GoRuntime retrieves the current runtime data
func GoRuntime() string {
	return getData().Runtime
}

// BuildDate extracts the build date from the build flags and formats the date in rfc 3339 format
func BuildDate() string {
	return getData().BuildDate.Format(time.RFC3339)
}

// CodeRepo extracts the repository portion of the owner/repo build flag
func CodeRepo() string {
	_, repo := parseCodeRepoRaw()
	return repo
}

// CodeOwner extracts the owner portion of the owner/repo build flag
func CodeOwner() string {
	owner, _ := parseCodeRepoRaw()
	return owner

}

// returns a tuple of (owner, repo).
func parseCodeRepoRaw() (string, string) {
	if codeRepo == "" {
		matches := ownerRepoRegex.FindStringSubmatch(codeRepoRaw)
		if matches != nil {
			codeOwner, codeRepo = matches[1], matches[2]
		} else {
			codeOwner, codeRepo = UnknownCodeOwner, UnknownCodeRepo
		}
	}
	return codeOwner, codeRepo
}
