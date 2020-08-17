// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package network

import (
	"net/http"

	"github.com/theparanoids/ashirt-server/backend/dtos"
	"github.com/theparanoids/aterm/errors"
)

const errCannotConnectMsg = "Unable to connect to the server"

// GetOperations retrieves all of the operations that are exposed to backend tools (api routes)
func GetOperations() ([]dtos.Operation, error) {
	var ops []dtos.Operation

	resp, err := makeJSONRequest("GET", apiURL+"/operations", http.NoBody)
	if err != nil {
		return ops, errors.Wrap(err, errCannotConnectMsg)
	}

	if err = evaluateResponseStatusCode(resp.StatusCode); err != nil {
		return ops, err
	}

	err = readResponseBody(&ops, resp.Body)

	return ops, err
}
