// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package network_test

import (
	"bytes"
	"testing"
	"time"

	"github.com/stretchr/testify/require"
	"github.com/theparanoids/aterm/network"
)

func Now() string {
	return time.Now().Format(time.RFC3339)
}

func TestUpload(t *testing.T) {
	t.Skip("skipping network tests")
	var written []byte

	uploadInput := network.UploadInput{
		OperationSlug: "first",
		Description:   "abcd",
		ContentType:   network.ContentTypeTerminalRecording,
		Filename:      "dolphin",
		Content:       bytes.NewReader([]byte("abc123")),
	}

	makeServer(Route{"POST", "/api/operations/first/evidence", newRequestRecorder(201, `{"uuid": "0000", "description": "`+uploadInput.Description+`", "occurredAt": "`+Now()+`"}`, &written)})
	network.SetServer(network.MkTestServer(testPort))

	_, err := network.UploadToAshirt(uploadInput)

	require.Nil(t, err)
}

func TestUploadFailedWithJSONError(t *testing.T) {
	t.Skip("skipping network tests")
	var written []byte
	makeServer(Route{"POST", "/api/operations/second/evidence", newRequestRecorder(402, `{"error": "oops"}`, &written)})
	network.SetServer(network.MkTestServer(testPort))

	uploadInput := network.UploadInput{
		OperationSlug: "second",
		Description:   "abcd",
		ContentType:   network.ContentTypeTerminalRecording,
		Filename:      "dolphin",
		Content:       bytes.NewReader([]byte("abc123")),
	}

	_, err := network.UploadToAshirt(uploadInput)
	require.Error(t, err)
}

func TestUploadFailedWithUnknownJSON(t *testing.T) {
	t.Skip("skipping network tests")
	var written []byte
	makeServer(Route{"POST", "/api/operations/third/evidence", newRequestRecorder(402, `{"something": "value"}`, &written)})
	network.SetServer(network.MkTestServer(testPort))

	uploadInput := network.UploadInput{
		OperationSlug: "third",
		Description:   "abcd",
		ContentType:   network.ContentTypeTerminalRecording,
		Filename:      "dolphin",
		Content:       bytes.NewReader([]byte("abc123")),
	}

	_, err := network.UploadToAshirt(uploadInput)
	require.Error(t, err)
}
