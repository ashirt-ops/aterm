package network

import (
	"net/http"

	"github.com/theparanoids/ashirt-server/backend/dtos"
	"github.com/theparanoids/aterm/errors"
)

var ErrCannotConnect = errors.New("Unable to connect to the server")

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
