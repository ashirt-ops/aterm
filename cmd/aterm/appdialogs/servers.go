package appdialogs

import (
	"fmt"

	"github.com/theparanoids/aterm/cmd/aterm/config"
	"github.com/theparanoids/aterm/common"
	"github.com/theparanoids/aterm/dialog"
	"github.com/theparanoids/aterm/errors"
	"github.com/theparanoids/aterm/fancy"
	"github.com/theparanoids/aterm/network"
)

func askForServer() {
	createOpt := dialog.SimpleOption{Label: "<New>", Data: ""}

	allServers := config.GetAlphaSortedServers()
	selection := runSelectAServerDialog(allServers, []dialog.SimpleOption{createOpt})
	if !selection.IsValid() {
		return
	}

	if selection == createOpt {
		newServer, err := createServerDialog(common.NoServer, false)
		if err != nil {
			config.SetActiveServer(newServer.ServerUUID)
		}
	} else {
		val, ok := selection.Data.(string)
		if ok {
			config.SetActiveServer(val)
		} else {
			printline(fancy.Caution("That selection doesn't seem to be valid. This should be reported", nil))
		}
	}
}

func createServerDialog(modelServer common.Server, withPreamble bool) (common.Server, error) {
	stop := false
	doEdit := true
	backout := func() { stop = true }
	// ask is a tiny helper to generate an askFor message, with some common fields pre-set
	ask := func(fields AskForTemplateFields, defVal string, saveTo *string) {
		if stop {
			return
		}
		mods := AskForTemplateModifiers{WithPreamble: &withPreamble}
		result := realize(askFor(edit(fields, mods), &defVal, backout).Value)
		*saveTo = result
	}

	saveOpt := dialog.SimpleOption{Label: "Save"}
	editOpt := dialog.SimpleOption{Label: "Make Changes"}
	testOpt := dialog.SimpleOption{Label: "Test"}
	cancelOpt := dialog.SimpleOption{Label: "Cancel"}
	finishedOptions := []dialog.SimpleOption{saveOpt, editOpt, testOpt, cancelOpt}

	if withPreamble {
		printline("If the value in [brackets] looks good, simply press enter to accept that value.")
	}

	newServer := common.Server{}
	for doEdit {
		ask(serverNameFields, modelServer.ServerName, &newServer.ServerName)
		ask(apiURLFields, modelServer.HostPath, &newServer.HostPath)

		if withPreamble && !stop {
			printline("I need to know your credentials to talk to the ASHIRT servers. You can generate a new key from your account settings on the ASHIRT website.")
		}
		ask(accessKeyFields, modelServer.AccessKey, &newServer.AccessKey)
		ask(secretKeyFields, modelServer.SecretKey, &newServer.SecretKey)

		if stop {
			printline("Returning without making changes...")
			return common.NoServer, errors.ErrCancelled
		}

		printline("This is what I got:")
		serverSettingsTemplate.Execute(medium, newServer)

		err := newServer.ValidateServerConfig()
		if err != nil {
			ShowInvalidConfigMessageNoHelp(err)
		}

		resp := HandlePlainSelect("Should I save this?", finishedOptions,
			func() dialog.SimpleOption {
				printline("Cancelling...")
				return dialog.InvalidSelection
			}).Selection

		if resp == saveOpt {
			return config.UpsertServer(newServer)
		} else if resp == cancelOpt || resp == dialog.InvalidSelection {
			return common.NoServer, errors.ErrCancelled
		} else if resp == testOpt {
			var msg string
			var testErr error
			dialog.DoBackgroundLoading(dialog.SyncedFunc(func() {
				msg, testErr = network.TestCustomConnection(newServer)
			}))

			if testErr == nil {
				return config.UpsertServer(newServer)
			}
			printfln("%v Could not connect: %v", fancy.RedCross(), fancy.WithBold(testErr.Error(), fancy.Red))
			if msg != "" {
				printline("Recommendation: " + msg)
			}

			saveAnywayOpt := dialog.SimpleOption{Label: "Save Anyway"}
			postTestResp := HandlePlainSelect("What do you want to do?", []dialog.SimpleOption{saveAnywayOpt, editOpt, cancelOpt},
				func() dialog.SimpleOption {
					printline("Cancelling...")
					return dialog.InvalidSelection
				}).Selection

			if postTestResp == saveAnywayOpt {
				return config.UpsertServer(newServer)
			} else if postTestResp == cancelOpt || postTestResp == dialog.InvalidSelection {
				return common.NoServer, errors.ErrCancelled
			} else {
				modelServer = newServer // re-loop
			}
		} else if resp == editOpt {
			modelServer = newServer // re-loop
		}
	}

	return newServer, nil
}

func runSelectAServerDialog(allServers []common.Server, alwaysOptions []dialog.SimpleOption) dialog.SimpleOption {
	var currentServerOpt dialog.SimpleOption
	otherServers := make([]dialog.SimpleOption, 0, len(allServers))

	curServer := config.GetCurrentServer()
	if curServer.IsValidServer() {
		currentServerOpt = dialog.SimpleOption{
			Label: fmt.Sprintf("%v (current)", curServer.ServerName),
			Data:  curServer.ServerUUID,
		}
	}

	for _, server := range allServers {
		if server.ServerUUID == curServer.ServerUUID { //already added
			continue
		}

		otherServers = append(otherServers, dialog.SimpleOption{
			Label: server.GetServerName(),
			Data:  server.ServerUUID,
		})
	}

	allOptions := make([]dialog.SimpleOption, 0, 1+len(otherServers)+len(alwaysOptions))
	if curServer.IsValidServer() {
		allOptions = append(allOptions, currentServerOpt)
	}
	allOptions = append(allOptions, otherServers...)
	allOptions = append(allOptions, alwaysOptions...)

	resp := HandlePlainSelect("Select a server", allOptions, func() dialog.SimpleOption {
		printline("Keeping current server")
		return dialog.InvalidSelection
	})
	return resp.Selection
}

// SignalCurrentServerUpdate alerts the network package that the server has changed.
func SignalCurrentServerUpdate() {
	server := config.GetCurrentServer()
	if server.IsValidServer() {
		network.SetServer(server)
	}
}
