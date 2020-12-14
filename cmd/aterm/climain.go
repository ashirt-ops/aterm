// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package main

import (
	"os"

	"github.com/theparanoids/aterm/cmd/aterm/appdialogs"
	"github.com/theparanoids/aterm/cmd/aterm/config"
	"github.com/theparanoids/aterm/cmd/aterm/recording"

	"github.com/jrozner/go-info"
)

func main() {
	appdialogs.PrintVersion()
	err := config.Ready()

	printConfig := config.GetCLI().PrintConfig
	showMenu := config.GetCLI().ShowMenu

	if config.ShowPID() {
		appdialogs.PrintPID()
	}

	if info.Flag() || config.GetCLI().PrintVersion {
		appdialogs.PrintExtendedVersion()
		return
	}

	if config.IsNew() || config.GetCLI().ForceFirstRun {
		doFirstRun()
	} else if err != nil {
		appdialogs.ShowConfigurationParsingErrorMessage(err)
		printConfig = true
	}

	config.SetServer(config.ActiveServerUUID())

	if s := config.GetCurrentServer(); s.IsValidServer() {
		validationErr := s.ValidateServerConfig()
		if validationErr != nil {
			appdialogs.ShowInvalidConfigurationMessage(validationErr)
			showMenu = true
		} else {
			appdialogs.SignalCurrentServerUpdate()
		}
	}

	appdialogs.NotifyUpdate(config.Version(), config.CodeOwner(), config.CodeRepo())

	// Check CLI-derived flags
	if printConfig {
		config.PrintLoadedConfig(os.Stdout)
		os.Exit(-1)
	}

	recording.InitializeRecordings()

	menuState := appdialogs.MenuState{
		InstanceConfig: config.CurrentConfig(),
	}

	if showMenu {
		menuState.CurrentView = appdialogs.MenuViewMainMenu
	} else {
		menuState.CurrentView = appdialogs.MenuViewRecording
	}
	appdialogs.StartMenus(menuState)
}

func doFirstRun() {
	err := appdialogs.FirstRun(config.ATermConfigPath(), config.ASHIRTConfigPath())
	if err != nil {
		appdialogs.ShowFirstRunErrorMessage(err)
	}
}
