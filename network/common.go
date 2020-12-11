// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package network

import (
	"encoding/json"
	"fmt"
	"io"
	"io/ioutil"
	"net/http"
	"strings"
	"time"

	"github.com/theparanoids/ashirt-server/signer"
	"github.com/theparanoids/aterm/common"
	"github.com/theparanoids/aterm/errors"
)

var client = &http.Client{}

var currentServer common.Server = common.NoServer

// SetServer sets the connection details for connecting to the remote server
func SetServer(server common.Server) {
	currentServer = server
}

// IsServerSet checks if SetServer has been called
func IsServerSet() bool {
	return currentServer != common.NoServer
}

func getAPIUrl(domain string) string {
	if !strings.HasSuffix(domain, "/") {
		domain += "/"
	}
	domain += "api"
	return domain
}

func mkURL(endpoint string) string {
	return getAPIUrl(currentServer.HostPath) + endpoint
}

func mkCustomURL(domain, endpoint string) string {
	return getAPIUrl(domain) + endpoint
}

// addAuthentication adds Date and Authentication headers to the provided request
// returns an error if building an appropriate authentication value fails, nil otherwise
// Note: This should be called immediately before sending a request.
func addAuthentication(req *http.Request) error {
	return addArbitraryAuthentication(currentServer, req)
}

func addArbitraryAuthentication(server common.Server, req *http.Request) error {
	req.Header.Set("Date", time.Now().In(time.FixedZone("GMT", 0)).Format(time.RFC1123))
	authorization, err := signer.BuildClientRequestAuthorization(req, server.AccessKey, server.SecretKeyAsB64())
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
		return errors.ErrCannotConnect
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

	if err != nil {
		return nil, err
	}
	req.Header.Add("Content-Type", "application/json")

	if err = addAuthentication(req); err != nil {
		return nil, err
	}

	return client.Do(req)
}

// todo: maybe refactor makeJSONRequest and makeCustomJSONRequest to share a common base?
func makeCustomJSONRequest(method, endpoint string, server common.Server, body io.Reader) (*http.Response, error) {
	url := mkCustomURL(server.HostPath, endpoint)
	req, err := http.NewRequest(method, url, body)

	if err != nil {
		return nil, err
	}
	req.Header.Add("Content-Type", "application/json")

	if err = addArbitraryAuthentication(server, req); err != nil {
		return nil, err
	}

	return client.Do(req)
}
