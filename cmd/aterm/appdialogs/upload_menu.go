package appdialogs

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strings"

	"github.com/hashicorp/go-multierror"
	"github.com/theparanoids/ashirt-server/backend/dtos"
	"github.com/theparanoids/aterm/dialog"
	"github.com/theparanoids/aterm/errors"
	"github.com/theparanoids/aterm/fancy"
	"github.com/theparanoids/aterm/network"
)

func renderUploadMenu(state MenuState) MenuState {
	rtnState := state

	menuOptions := []dialog.SimpleOption{
		dialogOptionUploadRecording,
		dialogOptionRenameRecording,
		dialogOptionDiscardRecording,
		dialogOptionJumpToMainMenu,
	}

	selection, err := PlainSelect("What do you want to do", menuOptions)

	switch {
	case dialogOptionUploadRecording == selection:
		isValid, data := validateRecording(state.RecordedMetadata)
		if !isValid {
			break
		}
		newMetadata, doUpload := collectRecordingMetadata(state.RecordedMetadata)
		rtnState.RecordedMetadata = newMetadata
		if doUpload {
			newMetadata := uploadRecording(rtnState.RecordedMetadata, data)
			rtnState.RecordedMetadata = newMetadata
			if rtnState.RecordedMetadata.Uploaded {
				saveCompletedRecording(rtnState.RecordedMetadata)
				rtnState.CurrentView = MenuViewMainMenu
			}
		}

	case dialogOptionJumpToMainMenu == selection:
		saveCompletedRecording(rtnState.RecordedMetadata)
		rtnState.CurrentView = MenuViewMainMenu

	case dialogOptionDiscardRecording == selection:
		newMetadata := discardRecording(state.RecordedMetadata)
		rtnState.RecordedMetadata = newMetadata
		if !IsRecordingValid(newMetadata) {
			rtnState.CurrentView = MenuViewMainMenu
		}

	case dialogOptionRenameRecording == selection:
		newMetadata := renameRecording(state.RecordedMetadata)
		rtnState.RecordedMetadata = newMetadata

	case err != nil:
		println("I got an error handling that respone: " + fancy.WithBold(err.Error()))
	default:
		println("Hmm, I don't know how to handle that request. This is probably a bug. Could you please report this?")
	}

	return rtnState
}

func saveCompletedRecording(metadata RecordingMetadata) error {
	data, err := json.MarshalIndent(metadata, "", "  ")

	if err == nil {
		savePath := metadata.FilePath + ".recordingmeta.json"
		err = ioutil.WriteFile(savePath, data, 0600)
	}
	return err
}

func renameRecording(metadata RecordingMetadata) RecordingMetadata {
	rtnMetadata := metadata

	dir, originalName := filepath.Split(metadata.FilePath)
	filename, err := queryWithDefault("Enter a new filename", &originalName)

	if err != nil {
		println(fancy.Fatal("Unable to move file", err))
	} else if filename != originalName {
		if !strings.HasSuffix(filename, ".cast") {
			filename += ".cast"
		}
		newPath := filepath.Join(dir, filename)
		err = os.Rename(metadata.FilePath, newPath)
		if err != nil {
			println(fancy.Fatal("Unable to move file", err))
		} else {
			println("Moved recording to:" + fancy.WithBold(newPath))
			rtnMetadata.FilePath = newPath
		}
	}

	return rtnMetadata
}

func discardRecording(metadata RecordingMetadata) RecordingMetadata {
	rtnMetadata := metadata
	no := dialog.SimpleOption{Label: "No"}
	yes := dialog.SimpleOption{Label: "Yes"}

	selection, err := PlainSelect("Are you sure you want to delete this recording", []dialog.SimpleOption{no, yes})

	switch {
	case yes == selection:
		err := os.Remove(metadata.FilePath)
		if err != nil {
			println("Unable to delete recording at: " + fancy.WithBold(metadata.FilePath))
			println(fancy.Fatal("Error", err))
		}
		rtnMetadata = RecordingMetadata{}
	case no == selection:
		break
	case err != nil:
		println(fancy.Caution("I got an error handling that respone", err))
	default:
		println("Hmm, I don't know how to handle that request. This is probably a bug. Could you please report this?")
	}
	return rtnMetadata
}

func validateRecording(metadata RecordingMetadata) (bool, []byte) {
	var err error
	var data []byte
	dialog.DoBackgroundLoadingWithMessage("Validating file",
		dialog.SyncedFunc(func() {
			data, err = ioutil.ReadFile(metadata.FilePath)
		}),
	)

	if err != nil {
		println(fancy.Fatal("Couldn't validate file", err))
		return false, data
	}
	println(fancy.GreenCheck() + " File Validated")
	return true, data
}

func uploadRecording(metadata RecordingMetadata, content []byte) RecordingMetadata {
	rtnMetadata := metadata
	//TODO print summary of future upload

	doContinue, err := dialog.YesNoPrompt("Do you want to continue?", "", internalMenuState.DialogInput)
	if err != nil {
		println("I got an error handling that respone: " + fancy.WithBold(err.Error()))
		return rtnMetadata
	}
	if doContinue {
		input := network.UploadInput{
			OperationSlug: metadata.OperationSlug,
			Description:   metadata.Description,
			ContentType:   network.ContentTypeTerminalRecording,
			Filename:      filepath.Base(metadata.FilePath),
			TagIDs:        tagsToIDs(metadata.SelectedTags), // TODO: filter out what doesn't exist anymore
			Content:       bytes.NewReader(content),
		}
		dialog.DoBackgroundLoading(dialog.SyncedFunc(
			func() {
				_, err = network.UploadToAshirt(input)
			}),
		)
		if err != nil {
			println(fancy.Caution("Unable to upload recording", err))
		} else {
			println(fancy.GreenCheck() + " File uploaded")
			rtnMetadata.Uploaded = true
		}
	}

	return rtnMetadata
}

func collectRecordingMetadata(metadata RecordingMetadata) (RecordingMetadata, bool) {
	// collect data
	rtnMetadata := metadata
	continueUpload := true
	collectedErrors := multierror.Append(nil)
	collectedErrors.ErrorFormat = errors.MultiErrorPrintFormat
	var err error

	rtnMetadata.Description, err = UserQuery("Enter a description for this recording", &metadata.Description)
	collectedErrors = multierror.Append(collectedErrors, err)

	var serverTags []dtos.Tag
	dialog.DoBackgroundLoading(dialog.SyncedFunc(
		func() {
			serverTags, err = network.GetTags(metadata.OperationSlug)
		}),
	)
	if err != nil {
		println(fancy.Caution("Unable to get tags", err))
	} else {
		rtnMetadata.SelectedTags = askForTags(metadata.OperationSlug, serverTags, tagsToIDs(metadata.SelectedTags))
	}
	collectedErrors = multierror.Append(collectedErrors, err)

	if err := collectedErrors.ErrorOrNil(); err != nil {
		println(fancy.Caution("I had an issue collecting metadata", err))
		println("I will salvage what I can, and you can retry.")
		continueUpload = false
	}

	return rtnMetadata, continueUpload
}

func askForTags(operationSlug string, allTags []dtos.Tag, selectedTagIDs []int64) []dtos.Tag {
	doneOpt := dialog.SimpleOption{Label: "<Done>"}
	createOpt := dialog.SimpleOption{Label: "<New>"}

	for {
		selection := askForSingleTag(allTags, selectedTagIDs, []dialog.SimpleOption{doneOpt, createOpt})
		if !selection.IsValid() {
			return []dtos.Tag{}
		}

		if selection == doneOpt {
			break
		} else if selection == createOpt {
			newTag, err := askForNewTag(operationSlug, allTags)
			if err != nil {
				if err == ErrCancelled {
					println("Tag creation cancelled")
				} else if err == ErrAlreadyExists {
					toggleValue(&selectedTagIDs, newTag.ID)
				} else {
					println("Unable to create tag. Error: " + err.Error())
				}
			} else {
				allTags = append(allTags, *newTag)
				selectedTagIDs = append(selectedTagIDs, newTag.ID)
			}
		} else {
			val, ok := selection.Data.(int64)
			if ok {
				toggleValue(&selectedTagIDs, val)
			} else {
				println(fancy.Caution("That selection doesn't seem to be valid. This should be reported", nil))
			}
		}
	}

	submitTags := make([]dtos.Tag, 0, len(selectedTagIDs))
	for _, tagID := range selectedTagIDs {
		for _, tag := range allTags {
			if tagID == tag.ID {
				submitTags = append(submitTags, tag)
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
			return &t, ErrAlreadyExists
		}
	}

	if name == "" {
		return nil, ErrCancelled
	}
	return network.CreateTag(operationSlug, name, network.RandomTagColor())
}

func askForSingleTag(allTags []dtos.Tag, selectedTagIDs []int64, alwaysOptions []dialog.SimpleOption) dialog.SimpleOption {
	firstTagOptions := make([]dialog.SimpleOption, 0, len(selectedTagIDs))
	lastTagOptions := make([]dialog.SimpleOption, 0, len(allTags))
	selectedTagNames := make([]string, 0, len(selectedTagIDs))

	for _, tag := range allTags {
		added := false
		for _, selectedTagID := range selectedTagIDs {
			if tag.ID == selectedTagID {
				selectedTagNames = append(selectedTagNames, tag.Name)
				firstTagOptions = append(firstTagOptions, dialog.SimpleOption{
					Label: fmt.Sprintf("%v (Deselect)", tag.Name),
					Data:  tag.ID,
				})
				added = true
				break
			}
		}
		if !added {
			lastTagOptions = append(lastTagOptions, dialog.SimpleOption{
				Label: tag.Name,
				Data:  tag.ID,
			})
		}
	}

	allTagOptions := alwaysOptions
	allTagOptions = append(allTagOptions, firstTagOptions...)
	allTagOptions = append(allTagOptions, lastTagOptions...)

	msg := fmt.Sprintf("Choose your tags (Currently: %v)", fancy.WithBold(strings.Join(selectedTagNames, ", "), 0))
	selection, err := PlainSelect(msg, allTagOptions)
	if err != nil {
		println(fancy.Caution("I had a problem making that selection", err))
		return dialog.InvalidSelection
	}
	return selection
}

func tagsToIDs(tags []dtos.Tag) []int64 {
	tagIDs := make([]int64, len(tags))
	for i, tag := range tags {
		tagIDs[i] = tag.ID
	}
	return tagIDs
}

// findIndex does a linear search in the given haystack to determine if there is a maching value
// Returns the index of the match, if a match is found.
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
