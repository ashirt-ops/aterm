package config

import "github.com/theparanoids/aterm/common"

// CurrentConfig returns the loaded config file for general purpose use (note that several helpers)
// exist for these fields, so this function is likely not needed most of the time)
func CurrentConfig() Config {
	return loadedConfig
}

// SetConfig changes the running config to the one provided. This change is temporary -- the configuration file
// must be written to disk if these changes need to be permanent. See SaveConfig
func SetConfig(newConfig Config) {
	loadedConfig = newConfig
}

// HostPath retrieves the host path / API URL for the currently selected server
func HostPath() string {
	return loadedConfig.GetHostPath()
}

// OutputDir returns the output directory specified in the configuration file
func OutputDir() string {
	return loadedConfig.GetOutputDir()
}

// AccessKey retrieves the access key for the currently selected server
func AccessKey() string {
	return loadedConfig.GetAccessKey()
}

// SecretKey retrieves the secret key for the currently selected server
func SecretKey() string {
	return loadedConfig.GetSecretKey()
}

// RecordingShell retrieves the desired shell environment set via the CLI, or the config if the CLI
// option has not been defined
func RecordingShell() string {
	if GetCLI().RecordingShell != "" {
		return GetCLI().RecordingShell
	}
	return loadedConfig.GetRecordingShell()
}

func ShowPID() bool {
	return GetEnv().PrintPID || GetCLI().PrintPID
}

// Ready reads config data from multiple sources (CLI, env, connection info, settings, etc)
func Ready() error {
	opts := GetCLI()
	GetEnv()

	if !opts.HardReset {
		if err := LoadConfig(); err != nil {
			return err
		}
		if err := LoadServersFile(); err != nil {
			return err
		}
		LoadSettings() // best effort -- failing to load settings doesn't really matter
	}
	upgrade()
	return nil
}

func upgrade() {
	switch CurrentConfig().GetConfigVersion() {
	case 1:
		cfgv1 := GetConfigV1()
		// migrate connection info to servers
		server := GetServer(common.DefaultServerUUID)
		if !server.IsValidServer() {
			server = common.Server{
				ServerName: "default", ID: 1, ServerUUID: common.DefaultServerUUID,
			}
		}
		server.AccessKey = cfgv1.GetAccessKey()
		server.SecretKey = cfgv1.GetSecretKey()
		server.HostPath = cfgv1.GetHostPath()
		UpsertServer(server)
		SetActiveServer(common.DefaultServerUUID)
		SetServer(common.DefaultServerUUID)
		// change cfgv1 into a ConfigV2
		cfgv2 := newConfigV2()
		cfgv2.OutputDir = cfgv1.GetOutputDir()
		cfgv2.RecordingShell = cfgv1.GetRecordingShell()
		EnableV2Config(cfgv2)
		SaveConfig()
	}
}
