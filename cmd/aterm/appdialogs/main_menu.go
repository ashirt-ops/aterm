package appdialogs

import (
	"os"

	"github.com/theparanoids/ashirt-server/backend/dtos"
	"github.com/theparanoids/aterm/cmd/aterm/config"
	"github.com/theparanoids/aterm/cmd/aterm/recording"
	"github.com/theparanoids/aterm/dialog"
	"github.com/theparanoids/aterm/fancy"
	"github.com/theparanoids/aterm/network"
)

func renderMainMenu(state MenuState) MenuState {
	rtnState := state
	menuOptions := []dialog.SimpleOption{
		dialogOptionStartRecording,
		dialogOptionUpdateOps,
		dialogOptionTestConnection,
		dialogOptionEditRunningConfig,
		dialogOptionExit,
	}

	selection, err := PlainSelect("What do you want to do", menuOptions)
	switch {
	case dialogOptionStartRecording == selection:
		rtnState.CurrentView = MenuViewRecording

	case dialogOptionExit == selection:
		rtnState.CurrentView = MenuViewExit

	case dialogOptionTestConnection == selection:
		testConnection()

	case dialogOptionUpdateOps == selection:
		newOps, err := updateOperations()
		if err != nil {
			printline(fancy.Caution("Unable to retrieve operations list", err))
		} else {
			rtnState.AvailableOperations = newOps
		}

	case dialogOptionEditRunningConfig == selection:
		newConfig := editConfig(state.InstanceConfig)
		rtnState.InstanceConfig = newConfig

	case err != nil:
		printline(fancy.Caution("I got an error handling that respone", err))
	default:
		printline("Hmm, I don't know how to handle that request. This is probably a bug. Could you please report this?")
	}

	return rtnState
}

func startNewRecording(state MenuState) MenuState {
	rtnState := state

	// collect info
	if len(state.AvailableOperations) == 0 {
		printline(fancy.ClearLine("Unable to record: No operations available (Try refreshing operations)"))
		rtnState.CurrentView = MenuViewMainMenu
		return rtnState
	}

	opSlug := askForOperationSlug(state.AvailableOperations, state.InstanceConfig.OperationSlug)

	recordedMetadata := RecordingMetadata{
		OperationSlug: *opSlug,
	}
	// rtnState.InstanceConfig.OperationSlug = opSlug

	// reuse last tags, if they match the operation
	if recordedMetadata.OperationSlug == state.RecordedMetadata.OperationSlug {
		recordedMetadata.SelectedTags = state.RecordedMetadata.SelectedTags
	} else {
		recordedMetadata.SelectedTags = []dtos.Tag{}
	}

	rtnState.RecordedMetadata = recordedMetadata

	// start the recording
	rtnState.DialogInput = recording.DialogReader()
	output, err := recording.StartRecording(rtnState.RecordedMetadata.OperationSlug)

	if err != nil {
		printline(fancy.Fatal("Unable to record", err))
		rtnState.CurrentView = MenuViewMainMenu
		return rtnState
	}
	rtnState.RecordedMetadata.FilePath = output.FilePath
	rtnState.CurrentView = MenuViewUploadMenu

	return rtnState
}

func testConnection() {
	var testErr error
	var value string
	dialog.DoBackgroundLoading(
		dialog.SyncedFunc(func() {
			value, testErr = network.TestConnection()
		}),
	)
	if testErr != nil {
		printfln("%v Could not connect: %v", fancy.RedCross(), fancy.WithBold(testErr.Error(), fancy.Red))
		if value != "" {
			printline("Recommendation: " + value)
		}
		return
	}
	printfln("%v Connected", fancy.GreenCheck())
}

func updateOperations() ([]dtos.Operation, error) {
	var loadingErr error
	var ops []dtos.Operation
	dialog.DoBackgroundLoadingWithMessage("Retriving operations",
		dialog.SyncedFunc(func() {
			ops, loadingErr = network.GetOperations()
		}),
	)

	if loadingErr != nil {
		return []dtos.Operation{}, loadingErr
	}

	printf("Updated operations (%v total)\n", len(ops))
	return ops, nil
}

func editConfig(runningConfig config.TermRecorderConfig) config.TermRecorderConfig {
	rtnConfig := runningConfig
	overrideCfg := config.CloneConfigAsOverrides(runningConfig)

	overrideCfg.AccessKey = askFor(accessKeyFields, overrideCfg.AccessKey).Value
	overrideCfg.SecretKey = askFor(secretKeyFields, overrideCfg.SecretKey).Value
	overrideCfg.APIURL = askFor(apiURLFields, overrideCfg.APIURL).Value

	overrideCfg.RecordingShell = askFor(shellFields, thisOrThat(overrideCfg.RecordingShell, os.Getenv("SHELL"))).Value
	overrideCfg.OutputDir = askFor(savePathFields, overrideCfg.OutputDir).Value
	overrideCfg.OperationSlug = askForOperationSlug(internalMenuState.AvailableOperations, runningConfig.OperationSlug)

	newCfg := config.PreviewUpdatedInstanceConfig(runningConfig, overrideCfg)

	config.PrintConfigTo(newCfg, os.Stdout)
	err := config.ValidateConfig(newCfg)
	if err != nil {
		ShowInvalidConfigMessageNoHelp(err)
	}
	yesPermanently := dialog.SimpleOption{Label: "Yes, and save for next time"}
	yesTemporarily := dialog.SimpleOption{Label: "Yes, for now"}
	cancelSave := dialog.SimpleOption{Label: "Cancel"}

	saveChangesOptions := []dialog.SimpleOption{
		yesPermanently,
		yesTemporarily,
		cancelSave,
	}
	selection, err := PlainSelect("Do you want to save these changes", saveChangesOptions)

	switch {
	case yesPermanently == selection:
		config.SetConfig(newCfg)
		if err := config.WriteConfig(); err != nil {
			ShowUnableToSaveConfigErrorMessage(err)
		}
		fallthrough
	case yesTemporarily == selection:
		network.SetAccessKey(newCfg.AccessKey)
		network.SetSecretKey(newCfg.SecretKey)
		network.SetBaseURL(newCfg.APIURL)
		rtnConfig = newCfg

	case cancelSave == selection:
		break

	case err != nil:
		printline(fancy.Caution("I got an error handling that respone", err))
	default:
		printline("Hmm, I don't know how to handle that request. This is probably a bug. Could you please report this?")
	}

	return rtnConfig
}
