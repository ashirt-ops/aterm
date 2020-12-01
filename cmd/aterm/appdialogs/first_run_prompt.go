package appdialogs

import (
	"encoding/json"
	"io/ioutil"
	"os"

	"github.com/theparanoids/ashirt-server/backend/dtos"
	"github.com/theparanoids/aterm/cmd/aterm/config"
	"github.com/theparanoids/aterm/fancy"

	"github.com/theparanoids/aterm/dialog"
	"github.com/theparanoids/aterm/errors"
	"github.com/theparanoids/aterm/network"
)

// FirstRun collects configuration data when the application is run for the first time. Data can be
// loaded from an external source (other ASHIRT application). Each question is prefaced with a small
// description of what is needed
func FirstRun(primaryConfigFile, pathToCommonConfig string) (config.TermRecorderConfigOverrides, error) {
	printf("Hi and welcome to the ASHIRT Terminal Recorder. \n"+
		"\n"+
		"I couldn't read a configuration file (I looked here: %v). "+
		"I think this might be the first run of this application. "+
		"Before we begin recording, we need to configure this application.\n",
		fancy.WithBold(primaryConfigFile),
	)

	var configData config.TermRecorderConfigOverrides

	// try to read common config
	content, err := ioutil.ReadFile(pathToCommonConfig)
	if err == nil {
		err = json.Unmarshal(content, &configData)
		if err == nil {
			printf("I was able to load defaults from another ASHIRT application. Let's double check these values.\n\n")
		}
	}

	printline("If the value in [brackets] looks good, simply press enter to accept that value.")
	configData.APIURL = askFor(apiURLFields, configData.APIURL, firstRunBail).Value

	printline("I need to know your credentials to talk to the ASHIRT servers. You can generate a new key from your account settings on the ASHIRT website.")
	configData.AccessKey = askFor(accessKeyFields, configData.AccessKey, firstRunBail).Value
	configData.SecretKey = askFor(secretKeyFields, configData.SecretKey, firstRunBail).Value

	configData.OutputDir = askFor(savePathFields, thisOrThat(configData.OutputDir, defaultRecordingHome), firstRunBail).Value

	checkConnection := true

	for checkConnection {
		printf(fancy.ClearLine("Let's check the network connection\n"))

		network.SetBaseURL(*configData.APIURL)
		network.SetAccessKey(*configData.AccessKey)
		network.SetSecretKey(*configData.SecretKey)

		var testErr error
		dialog.DoBackgroundLoading(dialog.SyncedFunc(func() {
			_, testErr = network.TestConnection()
		}))
		if testErr == nil {
			printf("These configurations work.\n")
			checkConnection = false
		} else if errors.Is(testErr, errors.ErrConnectionNotFound) {
			printf("It looks like the server is not available or the address is wrong.\n")
			fix, err := dialog.YesNoPrompt("Do you want to try to fix this?", "", medium)
			if fix && err == nil {
				configData.APIURL = askFor(apiURLFields, configData.APIURL, firstRunBail).Value
			} else {
				checkConnection = false
			}
		} else if errors.Is(testErr, errors.ErrConnectionUnauthorized) {
			printf("The server did not accept your access and secret key.\n")
			fix, err := dialog.YesNoPrompt("Do you want to try to fix this?", "", medium)
			if fix && err == nil {
				configData.AccessKey = askFor(AskForNoPreamble(accessKeyFields), configData.AccessKey, firstRunBail).Value
				configData.SecretKey = askFor(AskForNoPreamble(secretKeyFields), configData.SecretKey, firstRunBail).Value
			} else {
				checkConnection = false
			}
		} else {
			printf("I got an error I wasn't expecting. It's: '%v'. "+
				"This may be due to a network issue with the ASHIRT servers or with your own connection. "+
				"Please try contacting an administrator for help.\n", testErr.Error())
			checkConnection = false
		}
	}

	printf("\nOkay, that should be enough data for now. "+
		"I will create a configuration file here: %v. "+
		"You can find additional configuration options there.\n\n", config.ATermConfigPath())

	return configData, nil
}

func askForOperationSlug(availableOps []dtos.Operation, currentOperationSlug string) dialog.SelectResponse {
	currentOpSelection := dialog.SimpleOption{Data: dtos.Operation{Slug: currentOperationSlug}}
	if len(availableOps) == 0 {
		return dialog.SelectResponse{Selection: currentOpSelection}
	}
	resp := HandlePlainSelect("Select an Operation", operationsToOptions(availableOps, currentOperationSlug), func() dialog.SimpleOption {
		printline("Using current value...")
		return currentOpSelection
	})

	return resp
}

func operationsToOptions(ops []dtos.Operation, primarySlug string) []dialog.SimpleOption {
	operationOptions := make([]dialog.SimpleOption, len(ops))
	firstIndex := -1
	for i, op := range ops {
		suffix := ""
		if op.Slug == primarySlug {
			suffix = fancy.AsBold(" (Current)")
			firstIndex = i
		}

		operationOptions[i] = dialog.SimpleOption{Label: op.Name + suffix, Data: op}
	}

	if firstIndex == -1 {
		return operationOptions
	}
	reordered := []dialog.SimpleOption{operationOptions[firstIndex]}
	reordered = append(reordered, operationOptions[0:firstIndex]...)
	reordered = append(reordered, operationOptions[firstIndex+1:len(operationOptions)]...)

	return reordered
}

func firstRunBail() {
	printline("Exiting without changes")
	os.Exit(1)
}
