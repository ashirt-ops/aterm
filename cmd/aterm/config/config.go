package config

import (
	"fmt"
	"io"
	"io/ioutil"
	"net/url"
	"os"
	"path"
	"path/filepath"
	"runtime"

	"github.com/OpenPeeDeeP/xdg"
	"github.com/kelseyhightower/envconfig"
	"github.com/theparanoids/aterm/errors"
	"github.com/theparanoids/aterm/fancy"
	"github.com/theparanoids/aterm/network"
	"gopkg.in/yaml.v2"

	"github.com/hashicorp/go-multierror"
)

// configHome corrects the xdg-equivalent config home directory from the OpenPeeDeeP/xdg library
// for windows (it is arguably correct in other instances)
func configHome() string {
	if runtime.GOOS == "windows" {
		return os.Getenv("LOCALAPPDATA")
	}
	return xdg.ConfigHome()
}

// ASHIRTConfigPath points to the configuration file used by the ASHIRT application
func ASHIRTConfigPath() string {
	return filepath.Join(configHome(), "ashirt", "config.json")
}

// ATermConfigPath points to where the terminal recorder config is located
func ATermConfigPath() string {
	return filepath.Join(configHome(), "aterm", "config.yaml")
}

// ParseConfig returns the parsed configuration, based on built-in defaults, config file values,
// system environment configuration, and CLI overrides, in that order. See ParseConfigNoOverrides
// for a version with CLI overrides.
func ParseConfig(overrides CLIOptions) error {
	cfg, err := ParseConfigNoOverrides()

	applyCLIOverrides(&cfg, overrides)

	SetConfig(cfg)

	return err
}

// ParseConfigNoOverrides returns the parsed configuration, based on built-in defaults,
// config file values and system environment configuration, in that order.
// Note that this parses and returns the configuration, rather than storing it for later use
func ParseConfigNoOverrides() (TermRecorderConfig, error) {
	cfg := TermRecorderConfigWithDefaults()

	fileParseErr := parseConfigFile(&cfg)
	envParseErr := cfg.parseEnv()

	return cfg, errors.Append(fileParseErr, envParseErr)
}

func parseConfigFile(cfg *TermRecorderConfig) error {
	f, err := os.Open(ATermConfigPath())
	defer f.Close()
	if err != nil {
		if os.IsNotExist(err) {
			return errors.ErrConfigFileDoesNotExist
		}
		return errors.Wrap(err, "Unable to read config file")
	}
	return cfg.parseFileContent(f)
}

// applyCLIOverrides provides a mechansim to alter the provided TermRecorderConfig for known CLIOptions
// values.
func applyCLIOverrides(cfg *TermRecorderConfig, overrides CLIOptions) {
	if cfg == nil {
		return
	}
	if overrides.OperationSlug != "" {
		(*cfg).OperationSlug = overrides.OperationSlug
	}
	if overrides.RecordingShell != "" {
		(*cfg).RecordingShell = overrides.RecordingShell
	}
}

// ValidateLoadedConfig is shorthand for calling ValidateConfig(loadedConfig). i.e. it validates
// the loaded configuration, rather than an arbitrary configuration
func ValidateLoadedConfig() error {
	return ValidateConfig(loadedConfig)
}

// ValidateConfig checks the provided configuration for issues. Current issues include:
// * AccessKey is set
// * SecretKey set and decodable
// * APIURL parsable
// Returns an error. This error is a go-multierror, and can indicate multiple errors. Errors can
// be checked via errors.Is function
func ValidateConfig(tConfig TermRecorderConfig) error {
	validationErr := multierror.Append(nil)
	validationErr.ErrorFormat = errors.MultiErrorPrintFormat

	if tConfig.AccessKey == "" {
		multierror.Append(validationErr, errors.ErrAccessKeyNotSet)
	}

	if tConfig.SecretKey == "" {
		multierror.Append(validationErr, errors.ErrSecretKeyNotSet)
	} else if err := network.SetSecretKey(tConfig.SecretKey); err != nil {
		multierror.Append(validationErr, errors.ErrSecretKeyMalformed)
	}

	if _, err := url.Parse(tConfig.APIURL); err != nil {
		multierror.Append(validationErr, errors.ErrAPIURLUnparsable)
	}

	return validationErr.ErrorOrNil()
}

type TermRecorderConfig struct {
	ConfigVersion  int64  `yaml:"configVersion"`
	APIURL         string `yaml:"apiURL"         split_words:"true" envconfig:"api_url"`
	OutputDir      string `yaml:"outputDir"      split_words:"true"`
	AccessKey      string `yaml:"accessKey"      split_words:"true"`
	SecretKey      string `yaml:"secretKey"      split_words:"true" envconfig:"secret_key"`
	OutputFileName string `yaml:"-"              split_words:"true"`
	OperationSlug  string `yaml:"operationSlug"  split_words:"true"`
	RecordingShell string `yaml:"recordingShell" split_words:"true"`
}

type TermRecorderConfigOverrides struct {
	APIURL         *string `json:"apiURL"`
	OutputDir      *string `json:"evidenceRepo"`
	AccessKey      *string `json:"accessKey"`
	SecretKey      *string `json:"secretKey"`
	OutputFileName *string
	OperationSlug  *string
	RecordingShell *string
}

func CloneConfigAsOverrides(cfg TermRecorderConfig) TermRecorderConfigOverrides {
	strPtr := func(s string) *string { return &s }
	return TermRecorderConfigOverrides{
		APIURL:         strPtr(cfg.APIURL),
		OutputDir:      strPtr(cfg.OutputDir),
		AccessKey:      strPtr(cfg.AccessKey),
		SecretKey:      strPtr(cfg.SecretKey),
		OutputFileName: strPtr(cfg.OutputFileName),
		OperationSlug:  strPtr(cfg.OperationSlug),
		RecordingShell: strPtr(cfg.RecordingShell),
	}
}

func CloneLoadedConfigAsOverrides() TermRecorderConfigOverrides {
	return CloneConfigAsOverrides(loadedConfig)
}

func (t *TermRecorderConfig) WriteConfigToFile(configFilePath string) error {
	os.MkdirAll(path.Dir(configFilePath), 0755)
	outFile, err := os.Create(configFilePath)
	if err != nil {
		return errors.Wrap(err, "Unable to create config file")
	}
	defer outFile.Close()

	if err = yaml.NewEncoder(outFile).Encode(t); err != nil {
		return errors.Wrap(err, "Unable to write config file")
	}
	return errors.MaybeWrap(outFile.Close(), "Could not close config file")
}

func WriteConfig() error {
	return loadedConfig.WriteConfigToFile(ATermConfigPath())
}

func (t *TermRecorderConfig) parseFileContent(reader io.Reader) error {
	bytes, err := ioutil.ReadAll(reader)
	if err != nil {
		return errors.Wrap(err, "Unable to read config file")
	}
	err = yaml.Unmarshal(bytes, &t)
	if err != nil {
		return errors.Wrap(err, "Unable to interpret config file as YAML document")
	}
	return nil
}

func (t *TermRecorderConfig) parseEnv() error {
	err := envconfig.Process("ASHIRT_TERM_RECORDER", t)
	if err != nil {
		return errors.Wrap(err, "Error reading env config")
	}
	return nil
}

func PrintLoadedConfig(w io.Writer) {
	PrintConfigTo(loadedConfig, w)
}

// PrintConfigTo writes the provided configuration to the provided io.Writer.
// This is optimized for human reading, rather than as a serialization format.
// All errors that are encountered while writing are ignored.
func PrintConfigTo(t TermRecorderConfig, w io.Writer) {
	PrintConfigWithHeaderTo("Current Configuration", t, w)
}

// PrintConfigWithHeaderTo writes the provided configuration to the provided io.Writer.
// This is optimized for human reading, rather than as a serialization format.
// All errors that are encountered while writing are ignored.
func PrintConfigWithHeaderTo(header string, t TermRecorderConfig, w io.Writer) {
	writeLine := func(s string) { w.Write([]byte(s + "\n\r")) }

	writeLine("\r" + fancy.Clear)
	writeLine(header + ":")
	writeLine(fmt.Sprintf("\tConfig Version:  %v", t.ConfigVersion))
	writeLine(fmt.Sprintf("\tAPI Host:        %v", t.APIURL))
	writeLine(fmt.Sprintf("\tOutput Base:     %v", t.OutputDir))
	writeLine(fmt.Sprintf("\tAccess Key:      %v", t.AccessKey))
	writeLine(fmt.Sprintf("\tSecret Key:      %v", t.SecretKey))
	writeLine(fmt.Sprintf("\tOutput Prefix:   %v", t.OutputFileName))
	writeLine(fmt.Sprintf("\tOperation Slug:  %v", t.OperationSlug))
	writeLine(fmt.Sprintf("\tRecording Shell: %v", t.RecordingShell))
}

// TermRecorderConfigWithDefaults generates a TermRecorderConfig struct with some common default values
func TermRecorderConfigWithDefaults() TermRecorderConfig {
	return TermRecorderConfig{
		ConfigVersion:  1,
		RecordingShell: os.Getenv("SHELL"),
	}
}
