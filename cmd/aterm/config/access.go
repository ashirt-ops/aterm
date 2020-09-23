package config

var loadedConfig TermRecorderConfig

func CurrentConfig() TermRecorderConfig {
	return loadedConfig
}

func SetConfig(newConfig TermRecorderConfig) {
	loadedConfig = newConfig
}

// PreviewUpdatedConfig provides a mechanism to generate a TermRecorderConfig without changing the
// loaded configuration. Can be chained with SetConfig to alter the current state of the application,
// and can modify the permanent state of the configuration with WriteConfig
func PreviewUpdatedConfig(overrides TermRecorderConfigOverrides) TermRecorderConfig {
	return PreviewUpdatedInstanceConfig(loadedConfig, overrides)
}

// PreviewUpdatedInstanceConfig provides a mechanism to generate a TermRecorderConfig without changing the
// provided configuration. Can be chained with SetConfig to alter the current state of the application,
// and can modify the permanent state of the configuration with WriteConfig
func PreviewUpdatedInstanceConfig(cfg TermRecorderConfig, overrides TermRecorderConfigOverrides) TermRecorderConfig {
	selectVal := func(newVal *string, oldVal string) string {
		if newVal != nil {
			return *newVal
		}
		return oldVal
	}

	return TermRecorderConfig{
		ConfigVersion:  cfg.ConfigVersion,
		APIURL:         selectVal(overrides.APIURL, cfg.APIURL),
		OutputDir:      selectVal(overrides.OutputDir, cfg.OutputDir),
		AccessKey:      selectVal(overrides.AccessKey, cfg.AccessKey),
		SecretKey:      selectVal(overrides.SecretKey, cfg.SecretKey),
		OutputFileName: selectVal(overrides.OutputFileName, cfg.OutputFileName),
		OperationSlug:  selectVal(overrides.OperationSlug, cfg.OperationSlug),
		RecordingShell: selectVal(overrides.RecordingShell, cfg.RecordingShell),
	}
}

// APIURL is an accessor for the currently loaded value of APIURL
func APIURL() string {
	return loadedConfig.APIURL
}

// OutputDir is an accessor for the currently loaded value of OutputDir
func OutputDir() string {
	return loadedConfig.OutputDir
}

// AccessKey is an accessor for the currently loaded value of AccessKey
func AccessKey() string {
	return loadedConfig.AccessKey
}

// SecretKey is an accessor for the currently loaded value of SecretKey
func SecretKey() string {
	return loadedConfig.SecretKey
}

// OutputFileName is an accessor for the currently loaded value of OutputFileName
func OutputFileName() string {
	return loadedConfig.OutputFileName
}

// OperationSlug is an accessor for the currently loaded value of OperationSlug
func OperationSlug() string {
	return loadedConfig.OperationSlug
}

// RecordingShell is an accessor for the currently loaded value of RecordingShell
func RecordingShell() string {
	return loadedConfig.RecordingShell
}
