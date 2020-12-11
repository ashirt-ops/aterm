package config

import (
	"os"
	"path/filepath"
	"runtime"

	"github.com/OpenPeeDeeP/xdg"
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

// ASHIRTServersPath points to the "servers" file used by the ASHIRT application.
func ASHIRTServersPath() string {
	return filepath.Join(configHome(), "ashirt", "servers.json")
}

// ATermConfigPath points to where the terminal recorder config is located
func ATermConfigPath() string {
	return filepath.Join(configHome(), "aterm", "config.yaml")
}

// ATermSettingsPath points to the terminal recorder settings (i.e. previous decisions to streamline application flow)
func ATermSettingsPath() string {
	return filepath.Join(configHome(), "aterm", "settings.json")
}

// ATermServersPath points to where the terminal recorder server config is located
func ATermServersPath() string {
	return filepath.Join(configHome(), "aterm", "servers.json")
}
