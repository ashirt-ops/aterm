// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package main

import (
	"fmt"
	"os"

	"github.com/theparanoids/aterm/cmd/aterm/appdialogs"
	"github.com/theparanoids/aterm/cmd/aterm/config"
	"github.com/theparanoids/aterm/cmd/aterm/recording"
	"github.com/theparanoids/aterm/errors"
	"github.com/theparanoids/aterm/fancy"
	"github.com/theparanoids/aterm/network"
)

func main() {
	// Parse CLI for overrides
	opts := config.ParseCLI()

	// Parse env/config file for base values
	var err error
	if opts.HardReset {
		// intentionally ignoring parsing the config file here
		opts.ForceFirstRun = true // force the creation of a new config file
	} else {
		err = config.ParseConfig(opts)
	}

	// Check if first run to set up configuration
	if errors.Is(err, config.ErrorConfigFileDoesNotExist) || opts.ForceFirstRun {
		configData, _ := appdialogs.FirstRun(config.ATermConfigPath(), config.ASHIRTConfigPath())
		config.SetConfig(config.PreviewUpdatedConfig(configData))
		config.WriteConfig()
	} else if err != nil {
		appdialogs.ShowConfigurationParsingErrorMessage(err)
		opts.PrintConfig = true
	}

	network.SetBaseURL(config.APIURL())
	network.SetAccessKey(config.AccessKey())

	validationErr := config.ValidateLoadedConfig()
	if validationErr != nil {
		appdialogs.ShowInvalidConfigurationMessage(validationErr)
		opts.ShowMenu = true
	}

	// Check CLI flags
	if opts.PrintConfig {
		config.PrintLoadedConfig(os.Stdout)
		return
	}

	if err := recording.InitializeRecordings(); err != nil {
		fmt.Println(fancy.Fatal("Got an error starting up", err))
		return
	}
	defer recording.RestoreTerminal()

	menuState := appdialogs.MenuState{
		InstanceConfig: config.CurrentConfig(),
	}

	if opts.ShowMenu {
		menuState.CurrentView = appdialogs.MenuViewMainMenu
	} else {
		menuState.CurrentView = appdialogs.MenuViewRecording
	}
	appdialogs.StartMenus(menuState)

}
