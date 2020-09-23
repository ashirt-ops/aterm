// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package network

import (
	"fmt"
	"net/http"

	"github.com/theparanoids/ashirt-server/backend/dtos"
	"github.com/theparanoids/aterm/errors"
)

var ErrCannotConnect = errors.New("Unable to connect to the server")
var ErrConnectionUnauthorized = errors.New("Could not connect: Unauthorized")
var ErrConnectionNotFound = errors.New("Could not connect: Not Found")
var ErrConnectionUnknownStatus = errors.New("Could not connect: Unknown status")

// GetOperations retrieves all of the operations that are exposed to backend tools (api routes)
func GetOperations() ([]dtos.Operation, error) {
	var ops []dtos.Operation

	resp, err := makeJSONRequest("GET", apiURL+"/operations", http.NoBody)
	if err != nil {
		return ops, errors.Append(err, ErrCannotConnect)
	}

	if err = evaluateResponseStatusCode(resp.StatusCode); err != nil {
		return ops, err
	}

	err = readResponseBody(&ops, resp.Body)

	return ops, err
}

// TestConnection performs a basic query to the backend and interprets the results.
// There are a few scenarios. A successful connection returns ("", nil)
// Otherwise, the return structure is ("suggestion to fix (if any)", underlyingError)
// the underlying error is likely (but not necessarily) one of:
// ErrConnectionUnknownStatus, ErrConnectionNotFound, ErrConnectionUnauthorized
// use errors.Is(err, target) to check these errors
func TestConnection() (string, error) {
	resp, err := makeJSONRequest("GET", apiURL+"/operations", http.NoBody)
	if err != nil {
		return "", err
	}
	statusCode := resp.StatusCode
	if statusCode == http.StatusOK {
		return "", nil
	} else if statusCode == http.StatusUnauthorized {
		return "Check API and Secret keys", ErrConnectionUnauthorized
	} else if statusCode == http.StatusNotFound {
		return "Check API URL", ErrConnectionNotFound
	} else {
		return "", fmt.Errorf("%w : Status Code: %v", ErrConnectionUnknownStatus, statusCode)
	}
}
