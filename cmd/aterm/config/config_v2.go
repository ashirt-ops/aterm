package config

import (
	"os"

	"gopkg.in/yaml.v2"
)

var configV2 TermRecorderConfigV2

type TermRecorderConfigV2 struct {
	ConfigVersion  int64  `yaml:"configVersion"` // 2
	RecordingShell string `yaml:"recordingShell"`
	OutputDir      string `yaml:"outputDir"`
}

// DeserializeV2 attempts to interpret the given byte slice as a TermRecorderConfigV2.
// If successful, the parsed value and a nil error are returned. Otherwise, a zero-value config and
// the associated error are returned
func DeserializeV2(data []byte) (TermRecorderConfigV2, error) {
	var someConfig TermRecorderConfigV2
	err := yaml.Unmarshal(data, &someConfig)
	return someConfig, err
}

// EnableV2Config sets the internal config representation to the provided config struct provided
func EnableV2Config(cfg TermRecorderConfigV2) {
	configV2 = cfg
	SetConfig(configV2)
}

func newConfigV2() TermRecorderConfigV2 {
	rtn, _ := DeserializeV2(makeV2DefaultConfigAsByteSlice())
	return rtn
}

// makeV2DefaultConfigAsByteSlice generates a TermRecorderConfig struct with some common default values,
// then serializes the result
func makeV2DefaultConfigAsByteSlice() []byte {
	return (&TermRecorderConfigV2{
		ConfigVersion:  2,
		RecordingShell: os.Getenv("SHELL"),
	}).Serialize()
}

// GetConfigV2 returns the raw TermRecorderConfig as a true object, rather than being accessed from
// the Config interface
func GetConfigV2() TermRecorderConfigV2 { return configV2 }

// ***** Config interface implementations *****

// Serialize (v2) converts the config into a yaml file
func (c TermRecorderConfigV2) Serialize() []byte {
	data, _ := yaml.Marshal(c)
	return data
}

// GetConfigVersion (v2) returns 2
func (c TermRecorderConfigV2) GetConfigVersion() int64   { return c.ConfigVersion }
func (c TermRecorderConfigV2) GetRecordingShell() string { return c.RecordingShell }
func (c TermRecorderConfigV2) GetOutputDir() string      { return c.OutputDir }

// GetHostPath (v2) returns current information from GetCurrentServer()
func (c TermRecorderConfigV2) GetHostPath() string { return GetCurrentServer().HostPath }

// GetAccessKey (v2) returns current information from GetCurrentServer()
func (c TermRecorderConfigV2) GetAccessKey() string { return GetCurrentServer().AccessKey }

// GetSecretKey (v2) returns current information from GetCurrentServer()
func (c TermRecorderConfigV2) GetSecretKey() string { return GetCurrentServer().SecretKey }

func (c TermRecorderConfigV2) PreviewConfigUpdates(changes EditableConfig) Config {
	c.RecordingShell = changes.RecordingShell
	c.OutputDir = changes.OutputDir
	return c
}
