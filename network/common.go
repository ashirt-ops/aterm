// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package network

import (
	"encoding/json"
	"fmt"
	"io"
	"io/ioutil"
	"net/http"
	"time"

	"github.com/theparanoids/ashirt-server/signer"
	"github.com/theparanoids/aterm/errors"
)

var client = &http.Client{}

var apiURL string
var accessKey string
var secretKey []byte

// SetBaseURL Sets the url to use as a base for all service contact
// Note: this function only requires the url to reach the frontend service.
// routes will be deduced from that.
func SetBaseURL(url string) {
	apiURL = url + "/api"
}

// BaseURLSet is a small check to verify that some value exists for the BaseURL
func BaseURLSet() bool {
	return apiURL != ""
}

// SetAccessKey sets the common access key for all API actions
func SetAccessKey(key string) {
	accessKey = key
}

// SetSecretKey sets the common secret key for all API actions
func SetSecretKey(key []byte) {
	secretKey = key
}

// addAuthentication adds Date and Authentication headers to the provided request
// returns an error if building an appropriate authentication value fails, nil otherwise
// Note: This should be called immediately before sending a request.
func addAuthentication(req *http.Request) error {
	req.Header.Set("Date", time.Now().In(time.FixedZone("GMT", 0)).Format(time.RFC1123))
	authorization, err := signer.BuildClientRequestAuthorization(req, accessKey, secretKey)
	if err != nil {
		return err
	}
	req.Header.Set("Authorization", authorization)
	return nil
}

func evaluateResponseStatusCode(code int) error {
	switch {
	case code == http.StatusUnauthorized:
		return fmt.Errorf("Unable to authenticate with server. Please check credentials")
	case code == http.StatusInternalServerError:
		return fmt.Errorf("Server encountered an error")
	}
	if code != http.StatusOK && code != http.StatusCreated {
		return fmt.Errorf(errCannotConnectMsg)
	}
	return nil
}

func readResponseBody(container interface{}, body io.Reader) error {
	content, err := ioutil.ReadAll(body)
	if err != nil {
		return errors.Wrap(err, "Unable to read response")
	}

	if err := json.Unmarshal(content, &container); err != nil {
		return errors.Wrap(err, "Unable to parse response")
	}

	return nil
}

func makeJSONRequest(method, url string, body io.Reader) (*http.Response, error) {
	req, err := http.NewRequest(method, url, body)

	req.Header.Add("Content-Type", "application/json")

	if err != nil {
		return nil, err
	}

	if err = addAuthentication(req); err != nil {
		return nil, err
	}

	return client.Do(req)
}
