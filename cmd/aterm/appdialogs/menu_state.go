// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package appdialogs

import (
	"io"

	"github.com/theparanoids/ashirt-server/backend/dtos"
	"github.com/theparanoids/aterm/cmd/aterm/config"
)

type MenuState struct {
	CurrentView         MenuView
	AvailableOperations []dtos.Operation
	DialogInput         io.ReadCloser
	RecordedMetadata    RecordingMetadata
	InstanceConfig      config.Config
}

var internalMenuState = MenuState{}

type RecordingMetadata struct {
	Uploaded      bool       `json:"uploaded"`
	FilePath      string     `json:"filePath"`
	OperationSlug string     `json:"operationSlug"`
	Description   string     `json:"description"`
	SelectedTags  []dtos.Tag `json:"selectedTags"`
}

// IsRecordingValid is a small helper function to determine if the last recording was "valid"
// Typically not important to call
func IsRecordingValid(metadata RecordingMetadata) bool {
	return metadata.FilePath != ""
}
