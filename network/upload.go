// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package network

import (
	"bytes"
	"encoding/json"
	"io"
	"io/ioutil"
	"mime/multipart"
	"net/http"

	"github.com/pkg/errors"
)

type UploadInput struct {
	OperationSlug string
	Description   string
	Filename      string
	TagIDs        []int64
	Content       io.Reader
}

const errCouldNotInitMsg = "Unable to initialize Request"

// UploadToAshirt uploads a terminal recording to the AShirt service. The remote service must
// be configured by calling network.SetBaseURL(string) before uploading.
func UploadToAshirt(ui UploadInput) error {
	url := apiURL + "/operations/" + ui.OperationSlug + "/evidence"
	body := &bytes.Buffer{}
	writer := multipart.NewWriter(body)
	jsonTags, _ := json.Marshal(ui.TagIDs)
	fields := map[string]string{
		"notes":       ui.Description,
		"contentType": "terminal-recording",
		"tagIds":      string(jsonTags),
	}
	for k, v := range fields {
		err := writer.WriteField(k, v)
		if err != nil {
			return err
		}
	}

	part, err := writer.CreateFormFile("file", ui.Filename)
	if err != nil {
		return errors.Wrap(err, errCouldNotInitMsg)
	}
	_, err = io.Copy(part, ui.Content)
	if err != nil {
		return errors.Wrap(err, "Could not copy content")
	}
	err = writer.Close()
	if err != nil {
		return errors.Wrap(err, errCouldNotInitMsg)
	}

	req, err := http.NewRequest("POST", url, body)
	if err != nil {
		return errors.Wrap(err, errCouldNotInitMsg)
	}
	req.Header.Set("Content-Type", writer.FormDataContentType())
	err = addAuthentication(req)
	if err != nil {
		return errors.Wrap(err, errCouldNotInitMsg)
	}

	resp, err := client.Do(req)

	if err != nil {
		return errors.Wrap(err, "Unable to send request")
	}
	if resp.StatusCode != 201 {
		defer resp.Body.Close()
		raw, err := ioutil.ReadAll(resp.Body)
		if err != nil {
			return errors.Wrap(err, "Server did not accept request: Unable to read error response")
		}
		var parsed map[string]string
		err = json.Unmarshal(raw, &parsed)
		if err != nil {
			return err
		}
		reason, ok := parsed["error"]
		if !ok {
			return errors.New("Unable to upload file")

		}
		return errors.New("Unable to upload file: " + reason)

	}

	return nil
}
