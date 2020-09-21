package network

import (
	"errors"
	"fmt"
	"net/http"

	"github.com/theparanoids/ashirt-server/backend/dtos"
)

var ErrConnectionUnauthorized = errors.New("Could not connect: Unauthorized")
var ErrConnectionNotFound = errors.New("Could not connect: Not Found")
var ErrConnectionUnknownStatus = errors.New("Could not connect: Unknown status")
var ErrOutOfDateServer = errors.New("Could not connect: Invalid or out of date server")

// TestConnection performs a basic query to the backend and interprets the results.
// There are a few scenarios. A successful connection returns ("", nil)
// Otherwise, the return structure is ("suggestion to fix (if any)", underlyingError)
// the underlying error is likely (but not necessarily) one of:
// ErrConnectionUnknownStatus, ErrConnectionNotFound, ErrConnectionUnauthorized
// use errors.Is(err, target) to check these errors
func TestConnection() (string, error) {
	resp, err := makeJSONRequest("GET", apiURL+"/checkconnection", http.NoBody)
	if err != nil {
		return "", err
	}
	statusCode := resp.StatusCode
	if statusCode == http.StatusOK {
		var cc dtos.CheckConnection
		if err = readResponseBody(&cc, resp.Body); err != nil || cc.Connected == false {
			return "Check API URL", ErrOutOfDateServer
		}

		return "", nil
	} else if statusCode == http.StatusUnauthorized {
		return "Check API and Secret keys", ErrConnectionUnauthorized
	} else if statusCode == http.StatusNotFound {
		return "Check API URL", ErrConnectionNotFound
	} else {
		return "", fmt.Errorf("%w : Status Code: %v", ErrConnectionUnknownStatus, statusCode)
	}
}
