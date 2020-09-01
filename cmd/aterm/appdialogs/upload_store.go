// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package appdialogs

import (
	"io"

	"github.com/theparanoids/aterm/isthere"
	"github.com/theparanoids/aterm/network"
)

type UploadDefaults struct {
	FilePath      string
	OperationSlug string
	Description   string
}

// var cancelledUpload uploadDefaults

// UploadDialog is the controlling "dialog"/interface that allows for uploading of the previously
// recorded terminal session to the remote server
//
// UploadDialog exposes the following fields:
// OperationSlug: The "name" of the operation on the ashirt site (i.e. what to associate the uploaded content to)
// FilePath: the name/location of the file to be uploaded
type uploadStore struct {
	PreferredOperationSlug string
	DialogInput            io.ReadCloser
	Operations             []network.Operation
	DefaultData            UploadDefaults
}

var uploadStoreData = uploadStore{}

func SetBasicUploadData(operationSlug string, inputSrc io.ReadCloser) {
	uploadStoreData = uploadStore{
		PreferredOperationSlug: operationSlug,
		DialogInput:            inputSrc,
		Operations:             uploadStoreData.Operations,
		DefaultData:            UploadDefaults{},
	}
}

func SetDefaultData(defaults UploadDefaults) {
	uploadStoreData.DefaultData = defaults
}

func LoadOperations() error {
	ops, err := network.GetOperations()
	if isthere.No(err) {
		uploadStoreData.Operations = ops
	}
	return err
}

// opIDFromSlug performs an in-memory search for the corresponding id for the given slug
func opIDFromSlug(needle string) int64 {
	for _, op := range uploadStoreData.Operations {
		if op.Slug == needle {
			return op.ID
		}
	}
	return 0
}

// opSlugFromID performs an in-memory search for the corresponding slug for the given id
func opSlugFromID(needle int64) string {
	for _, op := range uploadStoreData.Operations {
		if op.ID == needle {
			return op.Slug
		}
	}
	return ""
}
