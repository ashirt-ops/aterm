package appdialogs

import (
	"github.com/theparanoids/ashirt-server/backend/dtos"
	"github.com/theparanoids/aterm/cmd/aterm/config"
	"github.com/theparanoids/aterm/dialog"
	"github.com/theparanoids/aterm/fancy"
)

// MenuView is a wrapper around string to represent each of the various primary screens/menus
type MenuView string

const (
	// MenuViewRecording begins a new recording session
	MenuViewRecording MenuView = "Recording"
	// MenuViewMainMenu sends the user to the main menu
	MenuViewMainMenu MenuView = "MainMenu"
	// MenuViewUploadMenu sends the user to a post-recording menu
	MenuViewUploadMenu MenuView = "UploadMenu"
	// MenuViewExit leaves the applications
	MenuViewExit MenuView = "Exit"
)

var (
	// main menu options
	dialogOptionExit              = dialog.SimpleOption{Label: "Exit"}
	dialogOptionTestConnection    = dialog.SimpleOption{Label: "Test Connection"}
	dialogOptionUpdateOps         = dialog.SimpleOption{Label: "Refresh Operations"}
	dialogOptionStartRecording    = dialog.SimpleOption{Label: "Start a New Recording"}
	dialogOptionEditRunningConfig = dialog.SimpleOption{Label: "Update Settings"}
	dialogOptionChangeServer      = dialog.SimpleOption{Label: "Switch Servers"}

	// upload menu options
	dialogOptionJumpToMainMenu   = dialog.SimpleOption{Label: "Return to Main Menu"}
	dialogOptionUploadRecording  = dialog.SimpleOption{Label: "Upload Recording"}
	dialogOptionDiscardRecording = dialog.SimpleOption{Label: "Discard Recording"}
	dialogOptionRenameRecording  = dialog.SimpleOption{Label: "Rename Recording File"}
)

// StartMenus starts processing the internal menu state. This produces a run loop, but should
// be handled in the main thread
func StartMenus(initialState MenuState) {
	internalMenuState = initialState
	ops, err := updateOperations()
	if err != nil {
		printline(fancy.Caution("Unable to get operations", err))

		// if we've previously recorded, assume the current op is still available
		if defaultSlug := config.LastOperation(); defaultSlug != "" {
			internalMenuState.AvailableOperations = []dtos.Operation{
				dtos.Operation{
					Slug: defaultSlug,
					Name: defaultSlug,
				},
			}
		}
	} else {
		internalMenuState.AvailableOperations = ops
	}

	runMenu()
}

func runMenu() {
	var exit = false
	for !exit {
		var newState MenuState
		switch internalMenuState.CurrentView {
		case MenuViewMainMenu:
			newState = renderMainMenu(internalMenuState)
		case MenuViewUploadMenu:
			newState = renderUploadMenu(internalMenuState)
		case MenuViewRecording:
			newState = startNewRecording(internalMenuState)
		case MenuViewExit:
			exit = true
		}

		internalMenuState = newState
	}
}
