package config

import (
	"github.com/kelseyhightower/envconfig"
	"github.com/theparanoids/aterm/errors"
)

var loadedEnv EnvConfig

type EnvConfig struct {
	Loaded         bool
	LoadingError   error
	PrintPID       bool   `split_words:"true"`
	RecordingShell string `split_words:"true"`
	ServerName     string `split_words:"true"`
}

func (t *EnvConfig) parseEnv() error {
	err := envconfig.Process("ASHIRT_TERM_RECORDER", t)
	if err != nil {
		t.LoadingError = errors.Wrap(err, "Error reading env config")
		return t.LoadingError
	}
	return nil
}

// GetEnv reads the (expected) environment variables and stores the result. Subsequent calls will
// return the pre-parsed result
func GetEnv() EnvConfig {
	if !loadedEnv.Loaded {
		loadedEnv.parseEnv()
		loadedEnv.Loaded = true
	}
	return loadedEnv
}
