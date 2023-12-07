package appdialogs

import (
	"errors"
	"fmt"

	"github.com/theparanoids/aterm/cmd/aterm/config"
	"github.com/theparanoids/aterm/fancy"
)

var ErrCancelled = fmt.Errorf("Cancelled")
var ErrAlreadyExists = fmt.Errorf("Already Exists")

// ShowInvalidConfigurationMessage renders user-messaging when a validation error occurs.
// To actually validate the config, see config.ValidateConfig/ValidateLoadedConfig
func ShowInvalidConfigurationMessage(validationErr error) {
	if validationErr == nil {
		return
	}

	showAccessCorrection := ShowInvalidConfigMessageNoHelp(validationErr)

	printfln("These errors can be corrected by editing the configuration in the main menu, or "+
		"by editing the configuration file directly at %v.", fancy.WithBold(config.ATermConfigPath()))

	if showAccessCorrection {
		printline("If you have lost your access key pair, you can generate a new pair from the ASHIRT servers.")
	}
}

func ShowInvalidConfigMessageNoHelp(validationErr error) bool {
	if validationErr == nil {
		return false
	}
	hasAccessIssue := false

	printline("I detected problems with this configuration:")
	if errors.Is(validationErr, config.ErrAccessKeyNotSet) {
		printline(" * Access Key has not been set")
		hasAccessIssue = true
	}
	if errors.Is(validationErr, config.ErrSecretKeyNotSet) {
		printline(" * Secret Key has not been set")
		hasAccessIssue = true
	}
	if errors.Is(validationErr, config.ErrSecretKeyMalformed) {
		printline(" * Secret Key is invalid")
		hasAccessIssue = true
	}
	if errors.Is(validationErr, config.ErrAPIURLUnparsable) {
		printline(" * API URL is invalid")
	}
	printline()

	return hasAccessIssue
}

// ShowConfigurationParsingErrorMessage renders user-messaging when the configuration file has issues
// parsing. It is assumed that these errors are NOT file-does-not-exist errors, as this should
// indicate a first run
func ShowConfigurationParsingErrorMessage(err error) {
	printline("I had a problem parsing the configuration file:")
	printline(" " + fancy.WithPizzazz(err.Error(), fancy.Red))
	printline("Execution will continue, but some features may not work until the above issue is fixed")
}

func ShowUnableToSaveConfigErrorMessage(err error) {
	printline("I was unable to save the updated configuration data. I encountered this error:")
	printline(" " + fancy.WithPizzazz(err.Error(), fancy.Red))
	printline("Settings will be saved for this run, but will need to be reconfigured the next time you start.")
}
