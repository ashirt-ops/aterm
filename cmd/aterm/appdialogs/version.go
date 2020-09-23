package appdialogs

import (
	"github.com/theparanoids/aterm/cmd/aterm/config"
)

// PrintVersion prints a simple sentence that lists the current version and commit hash
// Note: this should only be called _after_ parsing CLI options
func PrintVersion() {
	printfln("ATerm Version: %v  Build Hash: %v %v", config.Version(), config.CommitHash(), config.BuildDate())
}
