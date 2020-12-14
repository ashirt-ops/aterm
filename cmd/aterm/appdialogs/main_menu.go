package appdialogs

import (
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
		dialogOptionChangeServer,
		dialogOptionTestConnection,
		dialogOptionEditRunningConfig,
		dialogOptionExit,
	}

	printline("Current Server: " + fancy.WithPizzazz(config.GetCurrentServer().GetServerName(), fancy.Bold|fancy.Blue))
	resp := HandlePlainSelect("What do you want to do", menuOptions, func() dialog.SimpleOption {
		printline("Exiting...")
		return dialogOptionExit
	})
	switch {
	case dialogOptionStartRecording == resp.Selection:
		rtnState.CurrentView = MenuViewRecording

	case dialogOptionExit == resp.Selection:
		rtnState.CurrentView = MenuViewExit

	case dialogOptionTestConnection == resp.Selection:
		testConnection()

	case dialogOptionChangeServer == resp.Selection:
		askForServer()
		SignalCurrentServerUpdate()

	case dialogOptionUpdateOps == resp.Selection:
		newOps, err := updateOperations()
		if err != nil {
			printline(fancy.Caution("Unable to retrieve operations list", err))
		} else {
			rtnState.AvailableOperations = newOps
		}

	case dialogOptionEditRunningConfig == resp.Selection:
		newConfig := editConfig()
		rtnState.InstanceConfig = newConfig
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

	resp := askForOperationSlug(state.AvailableOperations, config.LastOperation())

	if resp.IsKillSignal() {
		rtnState.CurrentView = MenuViewMainMenu
		return rtnState
	}

	recordedMetadata := RecordingMetadata{
		OperationSlug: unwrapOpSlug(resp),
	}

	// reuse last tags, if they match the operation
	if recordedMetadata.OperationSlug == config.LastOperation() {
		recordedMetadata.SelectedTags = state.RecordedMetadata.SelectedTags
	} else {
		config.SetLastUsedOperation(recordedMetadata.OperationSlug)
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

func editConfig() config.Config {
	overrideCfg := config.CloneConfig()

	stop := false
	backout := func() { stop = true }
	ask := func(fields AskForTemplateFields, defVal string, saveFunc func(resp dialog.QueryResponse)) {
		if stop {
			return
		}
		result := askFor(fields, &defVal, backout)
		saveFunc(result)
	}

	ask(shellFields, overrideCfg.RecordingShell, func(q dialog.QueryResponse) {
		if !q.IsKillSignal() && q.Value != nil {
			overrideCfg.RecordingShell = *q.Value
		}
	})
	ask(savePathFields, overrideCfg.OutputDir, func(resp dialog.QueryResponse) {
		overrideCfg.OutputDir = realize(resp.Value)
	})
	if stop {
		printline("Discarding changes...")
		return config.CurrentConfig()
	}

	newCfg := config.CurrentConfig().PreviewConfigUpdates(overrideCfg)
	config.PrintConfigTo(newCfg, medium)

	yesPermanently := dialog.SimpleOption{Label: "Yes"}
	cancelSave := dialog.SimpleOption{Label: "Cancel"}

	saveChangesOptions := []dialog.SimpleOption{
		yesPermanently,
		cancelSave,
	}
	resp := HandlePlainSelect("Do you want to save these changes", saveChangesOptions, func() dialog.SimpleOption {
		printline("Discarding changes...")
		return cancelSave
	})

	switch {
	case yesPermanently == resp.Selection:
		config.SetConfig(newCfg)
		if err := config.SaveConfig(); err != nil {
			ShowUnableToSaveConfigErrorMessage(err)
		}
		return newCfg

	case cancelSave == resp.Selection:
		break

	default:
		printline("Hmm, I don't know how to handle that request. This is probably a bug. Could you please report this?")
	}

	return config.CurrentConfig()
}

func unwrapOpSlug(selectOpResp dialog.SelectResponse) string {
	if op, ok := selectOpResp.Selection.Data.(dtos.Operation); ok {
		return op.Slug
	}
	return ""
}
