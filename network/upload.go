// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package network

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"io/ioutil"
	"mime/multipart"
	"net/http"

	"github.com/theparanoids/ashirt-server/backend/dtos"
	"github.com/theparanoids/aterm/errors"
)

const (
	// ContentTypeTerminalRecording is the content type used for any terminal recording
	ContentTypeTerminalRecording = "terminal-recording"
	// ContentTypeScreenshot is the content type used for images (Screenshot or otherwise)
	ContentTypeScreenshot = "image"
	// ContentTypeCodeblock is the content type used for code blocks/text-based content
	ContentTypeCodeblock = "codeblock"
	// ContentTypeNone is the content type used for no-content evidence (i.e. description only)
	ContentTypeNone = "none"
)

// UploadInput provides a manifest for outgoing evidence.
type UploadInput struct {
	OperationSlug string
	Description   string
	ContentType   string
	Filename      string
	TagIDs        []int64
	Content       io.Reader
}

// UploadToAshirt uploads a terminal recording to the AShirt service. The remote service must
// be configured by calling network.SetBaseURL(string) before uploading.
func UploadToAshirt(ui UploadInput) (*dtos.Evidence, error) {
	url := mkURL("/operations/" + ui.OperationSlug + "/evidence")
	body := &bytes.Buffer{}
	writer := multipart.NewWriter(body)
	jsonTags, _ := json.Marshal(ui.TagIDs)
	fields := map[string]string{
		"notes":       ui.Description,
		"contentType": ui.ContentType,
		"tagIds":      string(jsonTags),
	}
	for k, v := range fields {
		err := writer.WriteField(k, v)
		if err != nil {
			return nil, err
		}
	}

	const couldNotInitMsg = "Unable to initialize Request"

	part, err := writer.CreateFormFile("file", ui.Filename)
	if err != nil {
		return nil, errors.Wrap(err, couldNotInitMsg)
	}
	_, err = io.Copy(part, ui.Content)
	if err != nil {
		return nil, errors.Wrap(err, "Could not copy content")
	}
	err = writer.Close()
	if err != nil {
		return nil, errors.Wrap(err, couldNotInitMsg)
	}

	req, err := http.NewRequest("POST", url, body)
	if err != nil {
		return nil, errors.Wrap(err, couldNotInitMsg)
	}
	req.Header.Set("Content-Type", writer.FormDataContentType())
	err = addAuthentication(req)
	if err != nil {
		return nil, errors.Wrap(err, couldNotInitMsg)
	}

	resp, err := client.Do(req)

	if err != nil {
		return nil, errors.Wrap(err, "Unable to send request")
	}
	if resp.StatusCode != 201 {
		defer resp.Body.Close()
		raw, err := ioutil.ReadAll(resp.Body)
		if err != nil {
			return nil, errors.Wrap(err, "Server did not accept request: Unable to read error response")
		}
		var parsed map[string]string
		err = json.Unmarshal(raw, &parsed)
		if err != nil {
			return nil, err
		}
		reason, ok := parsed["error"]
		if !ok {
			reason = "(unknown server error)"
		}
		return nil, fmt.Errorf("Unable to upload file: " + reason)
	}
	var evi dtos.Evidence
	return &evi, errors.MaybeWrap(readResponseBody(&evi, resp.Body), "Upload success, but unable to parse response")
}
