package errors

import (
	"fmt"
)

// ErrCancelled is a generic error to reflect the case where the user cancels an action
var ErrCancelled = fmt.Errorf("Cancelled")

// ErrAlreadyExists reflects the situation where a user tries to add a new item, but the item already exists
// specifically, this is used with Tags
var ErrAlreadyExists = fmt.Errorf("Already Exists")

// ErrFileDoesNotExist is the error returned when a file cannot be found at the
// time of (attempted) parsing
var ErrFileDoesNotExist = fmt.Errorf("File does not exist")

// ErrAccessKeyNotSet is the error returned when the provided access key is blank
var ErrAccessKeyNotSet = fmt.Errorf("Access Key has not been specified")

// ErrSecretKeyNotSet is the error returned when the provided secret key is blank
var ErrSecretKeyNotSet = fmt.Errorf("Secret Key has not been specified")

// ErrSecretKeyMalformed is the error returned when the provided secret key cannot be base64-decoded
var ErrSecretKeyMalformed = fmt.Errorf("Secret Key is malformed")

// ErrAPIURLUnparsable is the error returned when the given APIURL cannot be parsed
var ErrAPIURLUnparsable = fmt.Errorf("Unable to parse API URL")

// ErrCannotConnect reflects network errors where the server is unreachable
var ErrCannotConnect = fmt.Errorf("Unable to connect to the server")

// ErrConnectionUnauthorized reflects network errors where the server rejects the request due to
// bad/missing credentials
var ErrConnectionUnauthorized = fmt.Errorf("Could not connect: Unauthorized")

// ErrConnectionNotFound reflects network errors where there is no server at the address listed
var ErrConnectionNotFound = fmt.Errorf("Could not connect: Not Found")

// ErrConnectionUnknownStatus is a catch-all network error for situations where server communication
// cannot occur -- a last resort
var ErrConnectionUnknownStatus = fmt.Errorf("Could not connect: Unknown status")

// ErrOutOfDateServer reflects network issues where the remote server does not have the api requested
var ErrOutOfDateServer = fmt.Errorf("Could not connect: Invalid or out of date server")

// ErrNotInitialized reflects the situation that a pty recording was requsted, but the pty was not
// ready
var ErrNotInitialized = fmt.Errorf("Recordings have not been initialized")

// ErrServerNotFound is  TODO
var ErrServerNotFound = fmt.Errorf("Server entry does not exist")

// ErrInvalidServer is returned when a given common.Server has invalid data (specifically, if it is
// equivalent to common.NoServer)
var ErrInvalidServer = fmt.Errorf("Server is not valid")

// ErrUnknownConfigFormat TODO
var ErrUnknownConfigFormat = fmt.Errorf("Unable to interpret config file")

// ErrUnknownConfigVersion TOOD
var ErrUnknownConfigVersion = fmt.Errorf("Unknown config file version")
