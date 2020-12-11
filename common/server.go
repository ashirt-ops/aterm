package common

import (
	"encoding/base64"
	"net/url"

	"github.com/hashicorp/go-multierror"
	"github.com/theparanoids/aterm/errors"
)

const DefaultServerUUID = "20a28c7c-ea24-4ee0-bb94-0ee63018d34b"

// NoServer is a preset Server that has zero values, but is distinct from a freshly created Server object
var NoServer Server = Server{invalid: true}

// Server contains all of the data for a single entry in the servers.json file
type Server struct {
	invalid    bool
	ID         int64  `json:"id"`
	ServerName string `json:"serverName"`
	ServerUUID string `json:"serverUuid"`
	AccessKey  string `json:"accessKey"`
	SecretKey  string `json:"secretKey"`
	HostPath   string `json:"hostPath"`
	Deleted    bool   `json:"deleted"`
}

// IsValidServer provides a check to determine if the Server is a known server (read from servers file),
// or the NoServer
func (s Server) IsValidServer() bool {
	return s != NoServer
}

// GetServerName provides access to the ServerName field, or, if this is the NoServer,
// a "No Server Selected" message
func (s Server) GetServerName() string {
	if s == NoServer {
		return "No Server Selected"
	}
	return s.ServerName
}

func (s Server) ValidateServerConfig() error {
	validationErr := multierror.Append(nil)
	validationErr.ErrorFormat = errors.MultiErrorPrintFormat

	if s.AccessKey == "" {
		multierror.Append(validationErr, errors.ErrAccessKeyNotSet)
	}

	if s.SecretKey == "" {
		multierror.Append(validationErr, errors.ErrSecretKeyNotSet)
	} else if _, err := s.DecodeSecretKey(); err != nil {
		multierror.Append(validationErr, errors.ErrSecretKeyMalformed)
	}

	if _, err := url.Parse(s.HostPath); err != nil {
		multierror.Append(validationErr, errors.ErrAPIURLUnparsable)
	}

	return validationErr.ErrorOrNil()
}

// DecodeSecretKey decodes a the secret key associated with this server. This will return the base64
// decoded secret key, and an error if the string was not a (standard) base 64 string. The error
// should be checked prior to using the decoded value
// see SecretKeyAsB64
func (s Server) DecodeSecretKey() ([]byte, error) {
	return base64.StdEncoding.DecodeString(s.SecretKey)
}

// SecretKeyAsB64 returns the base64 encoding of the secret key (necessary for backend communication)
// if the secret key is not a valid base64 encoding, then an empty byte slice is returned.
// see DecodeSecretKey
func (s Server) SecretKeyAsB64() []byte {
	code, err := s.DecodeSecretKey()
	// note that per the base64.StdEncoding.DecodeString, a partial result may be returned,
	// so explcitly returning an empty slice here
	if err != nil {
		return []byte{}
	}
	return code
}
