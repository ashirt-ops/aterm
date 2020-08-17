// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package appdialogs

import (
	"bytes"
	"fmt"
	"io/ioutil"
	"path/filepath"
	"strings"
	"sync"

	"github.com/theparanoids/ashirt-server/backend/dtos"
	"github.com/theparanoids/aterm/dialog"
	"github.com/theparanoids/aterm/errors"
	"github.com/theparanoids/aterm/fancy"
	"github.com/theparanoids/aterm/network"
)

var errorCancelled = fmt.Errorf("Cancelled")
var errorAlreadyExists = fmt.Errorf("Already Exists")

var operationOptions = []dialog.Option{}

var menuOptionUpload = dialog.Option{Label: "Upload a file to the server", Action: showUploadSubmenu}
var menuOptionExit = dialog.Option{Label: "Exit", Action: dialog.MenuOptionGoBack}
var menuOptionUpdateOperations = dialog.Option{Label: "Refresh operations list", Action: updateOperationOptions}

// UserQuery is a re-packaging of dialog.UserQuery with inputStream pre-provided
func UserQuery(question string, defaultValue *string) (string, error) {
	return dialog.UserQuery(question, defaultValue, uploadStoreData.DialogInput)
}

// Select is a re-packaging of the dialog.Select with inpustStream pre-provided
func Select(label string, options []dialog.Option) dialog.OptionActionResponse {
	return dialog.Select(label, options, uploadStoreData.DialogInput)
}

// ShowUploadMainMenu presents the Main Menu during the uploading-of-capture phase
func ShowUploadMainMenu() {
	updateOperationOptions()

	mainMenuOptions := []dialog.Option{
		menuOptionUpdateOperations,
		menuOptionExit,
	}

	for {
		if !dialog.MenuContains(mainMenuOptions, menuOptionUpload) && len(operationOptions) > 0 {
			mainMenuOptions = append([]dialog.Option{menuOptionUpload}, mainMenuOptions...)
		}

		resp := Select("Select an operation", mainMenuOptions)
		if resp.ShouldExit {
			break
		}
		if resp.Err != nil {
			fmt.Println(fancy.Caution("Action failed", resp.Err))
		}
	}
}

func showUploadSubmenu() dialog.OptionActionResponse {
	err := tryUpload()
	if err != nil {
		switch errType := err.(type) {
		case CanceledOperation:
			SetDefaultData(errType.Data.(UploadDefaults))
			fmt.Println("Cancelled")
		default:
			fmt.Println("Encountered error during upload: " + err.Error())
		}
	}
	return dialog.NoAction()
}

func tryUpload() error {
	defaults := uploadStoreData.DefaultData
	if !network.BaseURLSet() {
		return fmt.Errorf("No service url specified -- check configuration")
	}

	path, err := UserQuery("Enter a filename", &defaults.FilePath)
	if err != nil {
		return errors.Wrap(err, "Could not retrieve filename")
	}

	fmt.Print("  Validating file... ")
	data, err := ioutil.ReadFile(path)

	if err != nil {
		return errors.Wrap(err, "Unable to read recording")
	}
	fmt.Println(fancy.ClearLine(fancy.GreenCheck()+" File Validated", 0))

	slugResp := Select("Enter an Operation Slug", operationOptions)
	if slugResp.Err != nil {
		return errors.Wrap(slugResp.Err, "Could not retrieve operation slug")
	}

	description, err := UserQuery("Enter a description for this recording", &defaults.Description)
	if err != nil {
		return errors.Wrap(err, "Could not retrieve description")
	}
	operationSlug := slugResp.Value.(string)
	tags, err := network.GetTags(operationSlug)
	var selectedTags []dtos.Tag
	if err != nil {
		fmt.Println("Unable to retrieve tags. This can be edited after submission on the website.")
	} else {
		selectedTags = askForTags(operationSlug, tags, []int64{})
	}

	// show a recap pre-upload
	fmt.Println(strings.Join([]string{
		fancy.WithBold("This will upload:", 0),
		fancy.WithBold("  File: ", 0) + fancy.WithBold(path, fancy.Yellow),
		fancy.WithBold("  Operation: ", 0) + fancy.WithBold(operationSlug, fancy.Yellow),
		fancy.WithBold("  Tags: ", 0) + fancy.WithBold(tagsToNames(selectedTags), fancy.Yellow),
		fancy.WithBold("  Description: ", 0) + fancy.WithBold(description, fancy.Yellow),
	}, "\n"))
	continueResp, err := dialog.YesNoPrompt("Do you want to continue?", "", uploadStoreData.DialogInput)
	if err != nil {
		return errors.Wrap(err, "Could not retrieve continue")
	}

	if continueResp == true {
		_, name := filepath.Split(path)

		input := network.UploadInput{
			OperationSlug: slugResp.Value.(string),
			Description:   description,
			ContentType:   network.ContentTypeTerminalRecording,
			Filename:      name,
			TagIDs:        tagsToIDs(selectedTags),
			Content:       bytes.NewReader(data),
		}

		var wg sync.WaitGroup
		wg.Add(1)
		stop := false
		var err error
		go func() {
			_, err = network.UploadToAshirt(input)
			wg.Done()
		}()
		go dialog.ShowLoadingAnimation("Loading", &stop)
		wg.Wait()
		stop = true
		fmt.Println(fancy.ClearLine(fancy.GreenCheck()+" File uploaded", 0))

		return errors.Wrap(err, "Could not upload")
	}

	return CanceledOperation{
		UploadDefaults{
			FilePath:      path,
			OperationSlug: slugResp.Value.(string),
			Description:   description,
		},
	}
}

func updateOperationOptions() (_ dialog.OptionActionResponse) {
	err := LoadOperations()
	if err != nil {
		fmt.Println(fancy.Caution("Unable to update operation list", err))
		return
	}

	operationOptions = make([]dialog.Option, len(uploadStoreData.Operations))
	for i, op := range uploadStoreData.Operations {
		operationOptions[i] = dialog.Option{
			Label:  op.Name,
			Action: dialog.ChooseAction(op.Slug),
		}
	}
	fmt.Printf("Loaded %v operations\n", len(uploadStoreData.Operations))
	return
}

func askForTags(operationSlug string, allTags []dtos.Tag, selectedTagIDs []int64) []dtos.Tag {
	const stopVal int64 = -1
	const createVal int64 = -2

	doneOpt := dialog.Option{Label: "<Done>", Action: dialog.ChooseAction(stopVal)}
	createOpt := dialog.Option{Label: "<New>", Action: dialog.ChooseAction(createVal)}

	for {
		selection := askForSingleTag(allTags, selectedTagIDs, []dialog.Option{doneOpt, createOpt})
		if selection.Err != nil {
			return []dtos.Tag{}
		}
		choice := selection.Value.(int64)

		if choice == stopVal {
			break
		} else if choice == createVal {
			newTag, err := askForNewTag(operationSlug, allTags)
			if err != nil {
				if err == errorCancelled {
					fmt.Println("Tag creation cancelled")
				} else if err == errorAlreadyExists {
					toggleValue(&selectedTagIDs, newTag.ID)
				} else {
					fmt.Println("Unable to create tag. Error: " + err.Error())
				}
			} else {
				allTags = append(allTags, *newTag)
				selectedTagIDs = append(selectedTagIDs, newTag.ID)
			}
		} else {
			toggleValue(&selectedTagIDs, choice)
		}
	}

	submitTags := make([]dtos.Tag, len(selectedTagIDs))
	for i, tagID := range selectedTagIDs {
		for _, tag := range allTags {
			if tagID == tag.ID {
				submitTags[i] = tag
			}
		}
	}

	return submitTags
}

func askForNewTag(operationSlug string, allTags []dtos.Tag) (*dtos.Tag, error) {
	name, err := UserQuery("Enter a new tag name", nil)
	if err != nil {
		return nil, err
	}
	lowerName := strings.ToLower(name)

	for _, t := range allTags {
		if lowerName == strings.ToLower(t.Name) {
			return &t, errorAlreadyExists
		}
	}

	if name == "" {
		return nil, errorCancelled
	}
	return network.CreateTag(operationSlug, name, network.RandomTagColor())
}

func askForSingleTag(allTags []dtos.Tag, selectedTagIDs []int64, alwaysOptions []dialog.Option) dialog.OptionActionResponse {
	firstTagOptions := make([]dialog.Option, 0, len(selectedTagIDs))
	lastTagOptions := make([]dialog.Option, 0, len(allTags))
	selectedTagNames := make([]string, 0, len(selectedTagIDs))

	for _, tag := range allTags {
		added := false
		for _, selectedTagID := range selectedTagIDs {
			if tag.ID == selectedTagID {
				selectedTagNames = append(selectedTagNames, tag.Name)
				firstTagOptions = append(firstTagOptions, dialog.Option{
					Label:  fmt.Sprintf("%v (Deselect)", tag.Name),
					Action: dialog.ChooseAction(tag.ID),
				})
				added = true
				break
			}
		}
		if !added {
			lastTagOptions = append(lastTagOptions, dialog.Option{
				Label:  tag.Name,
				Action: dialog.ChooseAction(tag.ID),
			})
		}
	}

	allTagOptions := alwaysOptions
	allTagOptions = append(allTagOptions, firstTagOptions...)
	allTagOptions = append(allTagOptions, lastTagOptions...)

	msg := fmt.Sprintf("Choose your tags (Currently: %v)", fancy.WithBold(strings.Join(selectedTagNames, ", "), 0))
	return Select(msg, allTagOptions)
}

func findIndex(haystack []int64, needle int64) int {
	for i := 0; i < len(haystack); i++ {
		if haystack[i] == needle {
			return i
		}
	}
	return -1
}

func toggleValue(numbs *[]int64, newNumb int64) {
	valIndex := findIndex(*numbs, newNumb)
	if valIndex > -1 {
		// swap found element with last element, then trim off the end (loses order)
		lastIndex := len(*numbs) - 1
		(*numbs)[valIndex], (*numbs)[lastIndex] = (*numbs)[lastIndex], (*numbs)[valIndex]
		*numbs = (*numbs)[:lastIndex]
	} else {
		*numbs = append(*numbs, newNumb)
	}
}

func tagsToNames(tags []dtos.Tag) string {
	selectedTagNames := make([]string, len(tags))
	for i, tag := range tags {
		selectedTagNames[i] = tag.Name
	}
	return strings.Join(selectedTagNames, ", ")
}

func tagsToIDs(tags []dtos.Tag) []int64 {
	tagIDs := make([]int64, len(tags))
	for i, tag := range tags {
		tagIDs[i] = tag.ID
	}
	return tagIDs
}
