package config

import (
	"fmt"
	"io"
	"io/ioutil"
	"os"

	"github.com/theparanoids/aterm/errors"
	"github.com/theparanoids/aterm/fancy"
)

var loadedConfig Config
var isNewConfig bool

// Config contains a general mechanism to access common data associated with configurations in general.
// Note that, as time progresses, some information may become deprecated
type Config interface {
	// Serialize provides a mechanism to convert a config into a binary-ish format (need not be binary,
	// but should be treated as such)
	Serialize() []byte

	// GetConfigVersion holds the version of the config file
	GetConfigVersion() int64
	// GetRecordingShell holds the preferred format for running the term recorder
	GetRecordingShell() string
	// GetOutputDir holds the information on where to save recordings (as a base path)
	GetOutputDir() string
	// GetHostPath holds the information on where to locate the server
	// Deprecated. Use config.GetCurrentServer().HostPath
	GetHostPath() string
	// GetAccessKey holds the information on how to authenticate with the server (akin to username)
	// Deprecated. Use config.GetCurrentServer().AccessKey
	GetAccessKey() string
	// GetSecretKey holds the information on how to authenticate with the server (akin to password)
	// Deprecated. Use config.GetCurrentServer().SecretKey
	GetSecretKey() string

	// UpdateConfig provides a set of changes the user wishes to apply to the config
	// Each config interface may choose which of these changes to apply (Some changes may not apply)
	PreviewConfigUpdates(changes EditableConfig) Config
}

func CloneConfig() EditableConfig {
	return EditableConfig{
		RecordingShell: CurrentConfig().GetRecordingShell(),
		OutputDir:      CurrentConfig().GetOutputDir(),
	}
}

func PrintLoadedConfig(w io.Writer) {
	PrintConfigTo(loadedConfig, w)
}

// PrintConfigTo writes the provided configuration to the provided io.Writer.
// This is optimized for human reading, rather than as a serialization format.
// All errors that are encountered while writing are ignored.
func PrintConfigTo(c Config, w io.Writer) {
	PrintConfigWithHeaderTo("Current Configuration", c, w)
}

// PrintConfigWithHeaderTo writes the provided configuration to the provided io.Writer.
// This is optimized for human reading, rather than as a serialization format.
// All errors that are encountered while writing are ignored.
func PrintConfigWithHeaderTo(header string, c Config, w io.Writer) {
	writeLine := func(s string) { w.Write([]byte(s + "\n\r")) }

	writeLine("\r" + fancy.Clear)
	writeLine(header + ":")
	writeLine(fmt.Sprintf("\tConfig Version:  %v", c.GetConfigVersion()))
	writeLine(fmt.Sprintf("\tHost Path:       %v", c.GetHostPath()))
	writeLine(fmt.Sprintf("\tOutput Base:     %v", c.GetOutputDir()))
	writeLine(fmt.Sprintf("\tAccess Key:      %v", c.GetAccessKey()))
	writeLine(fmt.Sprintf("\tSecret Key:      %v", c.GetSecretKey()))
	writeLine(fmt.Sprintf("\tRecording Shell: %v", c.GetRecordingShell()))
}

func SaveConfig() error {
	data := loadedConfig.Serialize()
	return writeFile(data, ATermConfigPath())
}

func LoadConfig() error {
	data, err := ioutil.ReadFile(ATermConfigPath())
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			data = makeV2DefaultConfigAsByteSlice()
			isNewConfig = true
		} else {
			return errors.Wrap(err, "Unable to load config file")
		}
	}
	psuedoConfig := interpretConfigData(data)
	if psuedoConfig.Interpreted {
		switch psuedoConfig.ConfigVersion {
		case 1:
			cfg, _ := DeserializeV1(data)
			EnableV1Config(cfg)
		case 2:
			cfg, _ := DeserializeV2(data)
			EnableV2Config(cfg)
		default:
			cfg, _ := DeserializeV2(makeV2DefaultConfigAsByteSlice())
			EnableV2Config(cfg) // set a basic config with no data
			return errors.ErrUnknownConfigVersion
		}
	} else {
		cfg, _ := DeserializeV2(makeV2DefaultConfigAsByteSlice())
		EnableV2Config(cfg) // set a basic config with no data
		return errors.ErrUnknownConfigFormat
	}

	return nil
}

func IsNew() bool {
	return isNewConfig
}
