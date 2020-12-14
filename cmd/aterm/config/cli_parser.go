package config

import (
	"flag"
)

var parsedCLI CLIOptions

// CLIOptions wraps the values that can be retrieved from the command line.
// Note that no-values are actually represented as zero-value
type CLIOptions struct {
	// Loaded is a simple check to see if the CLIOptions was loaded. if it was loaded, then this value will be true
	Loaded         bool
	RecordingShell string
	ShowMenu       bool
	PrintConfig    bool
	ForceFirstRun  bool
	HardReset      bool
	PrintVersion   bool
	PrintPID       bool
}

// GetCLI retrieves (and parses if necessary) all arguments from the command line. These values are
// stored for later use
func GetCLI() CLIOptions {
	if !parsedCLI.Loaded {
		parsedCLI = parseCLI()
	}
	return parsedCLI
}

func parseCLI() CLIOptions {
	var opts CLIOptions
	attachStringFlag("shell", "s", "Path to the shell to use for recording", "", &opts.RecordingShell)
	attachBoolFlag("menu", "m", "Show main menu", false, &opts.ShowMenu)
	attachBoolFlag("pid", "", "Print this process's PID", false, &opts.PrintPID)
	attachBoolFlag("print-config", "pc", "Print current configuration (post-command line arguments), then exits", false, &opts.PrintConfig)
	attachBoolFlag("reset", "", "Rerun first run to set up initial values", false, &opts.ForceFirstRun)
	attachBoolFlag("reset-hard", "", "Ignore the config file and rerun first run", false, &opts.HardReset)
	attachBoolFlag("v", "", "output the software version and build information", false, &opts.PrintVersion)
	flag.Parse()
	opts.Loaded = true
	return opts
}

// the below are small helpers to provide both short and long form flags -- not ideal, as it messes up
// the -h flag.

func attachStringFlag(longName, shortName, description, defaultValue string, variable *string) {
	if shortName != "" {
		flag.StringVar(variable, shortName, defaultValue, description)
	}
	flag.StringVar(variable, longName, defaultValue, description)
}

func attachIntFlag(longName, shortName, description string, defaultValue int64, variable *int64) {
	if shortName != "" {
		flag.Int64Var(variable, shortName, defaultValue, description)
	}
	flag.Int64Var(variable, longName, defaultValue, description)
}

func attachBoolFlag(longName, shortName, description string, defaultValue bool, variable *bool) {
	if shortName != "" {
		flag.BoolVar(variable, shortName, defaultValue, description)
	}
	flag.BoolVar(variable, longName, defaultValue, description)
}

func attachFloatFlag(longName, shortName, description string, defaultValue float64, variable *float64) {
	if shortName != "" {
		flag.Float64Var(variable, shortName, defaultValue, description)
	}
	flag.Float64Var(variable, longName, defaultValue, description)
}
