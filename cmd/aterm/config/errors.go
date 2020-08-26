package config

import "errors"

// ErrorConfigFileDoesNotExist is the error returned when the config file cannot be found at the
// time of (attempted) parsing
var ErrorConfigFileDoesNotExist = errors.New("Config file does not exist")

// ErrorAccessKeyNotSet is the error returned when the provided access key is blank
var ErrorAccessKeyNotSet = errors.New("Access Key has not been specified")

// ErrorSecretKeyNotSet is the error returned when the provided secret key is blank
var ErrorSecretKeyNotSet = errors.New("Secret Key has not been specified")

// ErrorSecretKeyMalformed is the error returned when the provided secret key cannot be base64-decoded
var ErrorSecretKeyMalformed = errors.New("Secret Key is malformed")

// ErrorAPIURLUnparsable is the error returned when the given APIURL cannot be parsed
var ErrorAPIURLUnparsable = errors.New("Unable to parse API URL")
