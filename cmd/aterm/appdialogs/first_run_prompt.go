package appdialogs

import (
	"encoding/json"
	"fmt"
	"io/ioutil"
	"os"

	"github.com/theparanoids/ashirt-server/backend/dtos"
	"github.com/theparanoids/aterm/cmd/aterm/config"
	"github.com/theparanoids/aterm/fancy"

	"github.com/theparanoids/aterm/dialog"
	"github.com/theparanoids/aterm/errors"
	"github.com/theparanoids/aterm/network"
)

func strPtr(s string) *string {
	return &s
}

var medium = os.Stdout

// FirstRun collects configuration data when the application is run for the first time. Data can be
// loaded from an external source (other ASHIRT application). Each question is prefaced with a small
// description of what is needed
func FirstRun(primaryConfigFile, pathToCommonConfig string) (config.TermRecorderConfigOverrides, error) {
	fmt.Printf("Hi and welcome to the ASHIRT Terminal Recorder.\n\n")
	fmt.Printf("I couldn't read a configuration file. (I looked here: %v). "+
		"I think this might be the first run of this application. "+
		"Before we begin recording, we need to configure this application.\n\n",
		primaryConfigFile)

	var configData config.TermRecorderConfigOverrides

	// try to read common config
	content, err := ioutil.ReadFile(pathToCommonConfig)
	if err == nil {
		err = json.Unmarshal(content, &configData)
		if err == nil {
			fmt.Printf("I was able to load defaults from another ASHIRT application. Let's double check these values.\n\n")
		}
	}

	fmt.Println("If the value in [brackets] looks good, simply press enter to accept that value.")
	configData.APIURL = askForAPIURL(configData.APIURL)
	configData.AccessKey, configData.SecretKey = askForAccessKeyAndSecret(configData.AccessKey, configData.SecretKey)
	configData.OutputDir = askForSavePath(configData.OutputDir)

	checkConnection := true

	for checkConnection {
		fmt.Printf(fancy.ClearLine("Let's check the network connection\n"))

		network.SetBaseURL(*configData.APIURL)
		network.SetAccessKey(*configData.AccessKey)
		network.SetSecretKey(*configData.SecretKey)

		var testErr error
		dialog.DoBackgroundLoading(dialog.SyncedFunc(func() {
			_, testErr = network.TestConnection()
		}))
		if testErr == nil {
			fmt.Printf("These configurations work.\n")
			checkConnection = false
		} else if errors.Is(testErr, network.ErrConnectionNotFound) {
			fmt.Printf("It looks like the server is not up or the address is wrong.\n")
			fix, err := dialog.YesNoPrompt("Do you want to try to fix this?", "", medium)
			if fix && err == nil {
				configData.APIURL = askForAPIURL(configData.APIURL)
			} else {
				checkConnection = false
			}
		} else if errors.Is(testErr, network.ErrConnectionUnauthorized) {
			fmt.Printf("The server did not accept your access and secret key.\n")
			fix, err := dialog.YesNoPrompt("Do you want to try to fix this?", "", medium)
			if fix && err == nil {
				configData.AccessKey, configData.SecretKey = askForAccessKeyAndSecret(configData.AccessKey, configData.SecretKey)
			} else {
				checkConnection = false
			}
		} else {
			fmt.Printf("I got an error I wasn't expecting. It's: '%v'. "+
				"This may be due to a network issue with the ASHIRT servers or with your own connection. "+
				"Please try contacting an administrator for help.\n", testErr.Error())
			checkConnection = false
		}
	}

	fmt.Printf("\nOkay, that should be enough data for now. "+
		"I will create a configuration file here: %v. "+
		"You can find additional configuration options there.\n\n", config.ATermConfigPath())

	return configData, nil
}

func askForAPIURL(guessValue *string) *string {
	fmt.Println("I need to know how to reach the ASHIRT servers. This typically looks like: " +
		"http://ashirt.company.com/api (though may not in your case). If you do not know, please contact your administrator.")

	apiURL, err := queryWithDefault("Enter the API URL", guessValue)

	if err != nil {
		fmt.Println(fancy.Caution("I had a problem collecting the API URL", err))
		return strPtr("")
	}

	return &apiURL
}

func askForAccessKeyAndSecret(apiKeyGuess, secretKeyGuess *string) (*string, *string) {
	fmt.Println("I need to know your credentials to talk to the ASHIRT servers. " +
		"You can generate a new key from your account settings on the ASHIRT website.")

	apiKeyAnswer, err1 := queryWithDefault("Enter the Access Key", apiKeyGuess)
	secretKeyAnswer, err2 := queryWithDefault("Enter the Secret Key", secretKeyGuess)

	if err := errors.Append(err1, err2); err != nil {
		fmt.Println(fancy.Caution("I had a problem collecting this info", err))
		return strPtr(""), strPtr("")
	}
	return &apiKeyAnswer, &secretKeyAnswer
}

func askForSavePath(guessValue *string) *string {
	fmt.Println("I need to know where to save the recordings. This can be anywhere on your computer but typically resides within a user's home directory.")

	if guessValue == nil {
		home := os.Getenv("HOME")
		guessValue = &home
	}
	path, err := queryWithDefault("Enter a save path", guessValue)

	if err != nil {
		fmt.Println(fancy.Caution("I had a problem collecting the save path", err))
		return strPtr("")
	}

	return &path
}

func askForShell(guessValue *string) *string {
	fmt.Println("I need to know what default shell to use to create the recordings. " +
		"This should be the absolute path to shell application. " +
		"If you know the name of the shell, but not the path, you can try running " +
		fancy.WithBold("which <shellName>") + " in a separate terminal.")

	shell, err := queryWithDefault("Enter the path to the shell", guessValue)

	if err != nil {
		fmt.Println(fancy.Caution("I had a problem collecting the save path", err))
		return strPtr("")
	}

	return &shell
}

func askForOperationSlug(availableOps []dtos.Operation, currentOperationSlug string) *string {
	if len(availableOps) == 0 {
		return &currentOperationSlug
	}
	selectedOpOption, err := PlainSelect("Select an Operation",
		operationsToOptions(availableOps, currentOperationSlug))

	if err != nil {
		fmt.Println(fancy.Caution("I had a problem getting the selected operation. Using the default instead.", err))
		return &currentOperationSlug
	}
	selectedOp := selectedOpOption.Data.(dtos.Operation) // ignoring type check here -- it should never fail

	return &selectedOp.Slug
}

func queryWithDefault(prompt string, guessValue *string) (string, error) {
	if guessValue != nil {
		prompt += " [" + *guessValue + "]"
	}

	answer, err := UserQuery(prompt, nil)
	if err != nil {
		return "", err
	}
	if answer == "" && guessValue != nil {
		return *guessValue, nil
	}
	return answer, nil
}
