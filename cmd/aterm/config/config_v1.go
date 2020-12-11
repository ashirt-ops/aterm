package config

import (
	"os"

	"gopkg.in/yaml.v2"
)

var configV1 TermRecorderConfig

type TermRecorderConfig struct {
	ConfigVersion  int64  `yaml:"configVersion"` // 1
	RecordingShell string `yaml:"recordingShell"`
	OutputDir      string `yaml:"outputDir"`
	APIURL         string `yaml:"apiURL"`
	AccessKey      string `yaml:"accessKey"`
	SecretKey      string `yaml:"secretKey"`
	OperationSlug  string `yaml:"operationSlug"`
}

// DeserializeV1 attempts to interpret the given byte slice as a TermRecorderConfig.
// If successful, the parsed value and a nil error are returned. Otherwise, a zero-value config and
// the associated error are returned
func DeserializeV1(data []byte) (TermRecorderConfig, error) {
	var someConfig TermRecorderConfig
	err := yaml.Unmarshal(data, &someConfig)
	return someConfig, err
}

// EnableV1Config sets the internal config representation to the provided config struct provided
func EnableV1Config(cfg TermRecorderConfig) {
	configV1 = cfg
	SetConfig(configV1)
}

// makeV1DefaultConfigAsByteSlice generates a TermRecorderConfig struct with some common default values, then
// serializes the result.
func makeV1DefaultConfigAsByteSlice() []byte {
	return (&TermRecorderConfig{
		ConfigVersion:  1,
		RecordingShell: os.Getenv("SHELL"),
	}).Serialize()
}

// GetConfigV1 returns the raw TermRecorderConfig as a true object, rather than being accessed from
// the Config interface
func GetConfigV1() TermRecorderConfig { return configV1 }

// ***** Config interface implementations *****

// Serialize (v1) converts the config into a yaml file
func (c TermRecorderConfig) Serialize() []byte {
	data, _ := yaml.Marshal(c)
	return data
}

// GetConfigVersion (v1) returns 1
func (c TermRecorderConfig) GetConfigVersion() int64   { return c.ConfigVersion }
func (c TermRecorderConfig) GetRecordingShell() string { return c.RecordingShell }
func (c TermRecorderConfig) GetOutputDir() string      { return c.OutputDir }

// GetHostPath (v1) returns the APIURL read from the v1 config file
func (c TermRecorderConfig) GetHostPath() string { return c.APIURL }

// GetAccessKey (v1) returns the access key read from the v1 config file
func (c TermRecorderConfig) GetAccessKey() string { return c.AccessKey }

// GetSecretKey (v1) returns the secre key read from the v1 config file
func (c TermRecorderConfig) GetSecretKey() string { return c.SecretKey }

func (c TermRecorderConfig) PreviewConfigUpdates(changes EditableConfig) Config {
	c.RecordingShell = changes.RecordingShell
	c.OutputDir = changes.OutputDir
	return c
}

// // applyCLIOverrides provides a mechansim to alter the provided TermRecorderConfig for known CLIOptions
// // values.
// func applyCLIOverrides(cfg *TermRecorderConfig, overrides CLIOptions) {
// 	if cfg == nil {
// 		return
// 	}
// 	if overrides.OperationSlug != "" {
// 		(*cfg).OperationSlug = overrides.OperationSlug
// 	}
// 	if overrides.RecordingShell != "" {
// 		(*cfg).RecordingShell = overrides.RecordingShell
// 	}
// }

// type TermRecorderConfigOverrides struct {
// 	APIURL         *string `json:"apiURL"`
// 	OutputDir      *string `json:"evidenceRepo"`
// 	AccessKey      *string `json:"accessKey"`
// 	SecretKey      *string `json:"secretKey"`
// 	OperationSlug  *string
// 	RecordingShell *string
// }

// func CloneConfigAsOverrides(cfg TermRecorderConfig) TermRecorderConfigOverrides {
// 	strPtr := func(s string) *string { return &s }
// 	return TermRecorderConfigOverrides{
// 		APIURL:         strPtr(cfg.APIURL),
// 		OutputDir:      strPtr(cfg.OutputDir),
// 		AccessKey:      strPtr(cfg.AccessKey),
// 		SecretKey:      strPtr(cfg.SecretKey),
// 		OperationSlug:  strPtr(cfg.OperationSlug),
// 		RecordingShell: strPtr(cfg.RecordingShell),
// 	}
// }
