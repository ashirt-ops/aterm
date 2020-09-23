package config

import "errors"

// ErrConfigFileDoesNotExist is the error returned when the config file cannot be found at the
// time of (attempted) parsing
var ErrConfigFileDoesNotExist = errors.New("Config file does not exist")

// ErrAccessKeyNotSet is the error returned when the provided access key is blank
var ErrAccessKeyNotSet = errors.New("Access Key has not been specified")

// ErrSecretKeyNotSet is the error returned when the provided secret key is blank
var ErrSecretKeyNotSet = errors.New("Secret Key has not been specified")

// ErrSecretKeyMalformed is the error returned when the provided secret key cannot be base64-decoded
var ErrSecretKeyMalformed = errors.New("Secret Key is malformed")

// ErrAPIURLUnparsable is the error returned when the given APIURL cannot be parsed
var ErrAPIURLUnparsable = errors.New("Unable to parse API URL")
