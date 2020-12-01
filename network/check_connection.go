package network

import (
	"fmt"
	"net/http"

	"github.com/theparanoids/ashirt-server/backend/dtos"
	"github.com/theparanoids/aterm/errors"
)

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
		if err = readResponseBody(&cc, resp.Body); err != nil || cc.Ok == false {
			return "Check API URL", errors.ErrOutOfDateServer
		}

		return "", nil
	} else if statusCode == http.StatusUnauthorized {
		return "Check API and Secret keys", errors.ErrConnectionUnauthorized
	} else if statusCode == http.StatusNotFound {
		return "Check API URL", errors.ErrConnectionNotFound
	} else {
		return "", fmt.Errorf("%w : Status Code: %v", errors.ErrConnectionUnknownStatus, statusCode)
	}
}
