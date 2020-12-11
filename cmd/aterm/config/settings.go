package config

import (
	"encoding/json"
	"io/ioutil"

	"github.com/theparanoids/aterm/errors"
)

var localSettings Settings
var cachedServerHistory map[string]ServerSettingHistory

// Settings reflects the internal state that wants to be preserved over multiple runs.
type Settings struct {
	ActiveServerUUID *string                `json:"selectedServerUuid"`
	ServerHistory    []ServerSettingHistory `json:"serverState"`
}

// ServerSettingHistory reflects the internal state at the last point the ServerUUID was used
type ServerSettingHistory struct {
	ServerUUID        string `json:"serverUuid"`
	LastOperationSlug string `json:"operationSlug"`
}

// LoadSettings attempts to read the settings file. If it can read, and parse, the file, then the
// settings will be stored in localSettings
func LoadSettings() error {
	data, err := ioutil.ReadFile(ATermSettingsPath())
	if err == nil {
		err = json.Unmarshal(data, &localSettings)
	}
	if err == nil {
		rebuildServerHistoryCache()
	}
	return err
}

// WriteSettings attempts to write the current settings to a file. Returns an error if writing fails
func WriteSettings() error {
	return writeSettings(localSettings)
}

func encodeCachedHistory() []ServerSettingHistory {
	history := make([]ServerSettingHistory, len(cachedServerHistory))
	i := 0
	for _, val := range cachedServerHistory {
		history[i] = val
		i++
	}
	return history
}

func writeSettings(settings Settings) error {
	hist := encodeCachedHistory()
	settings.ServerHistory = hist
	data, err := json.Marshal(settings)
	if err == nil {
		err = writeFile(data, ATermSettingsPath())
	}
	return errors.MaybeWrap(err, "Unable to create settings file")
}

func rebuildServerHistoryCache() {
	cachedServerHistory = make(map[string]ServerSettingHistory)
	for _, item := range localSettings.ServerHistory {
		cachedServerHistory[item.ServerUUID] = item
	}
}

// Settings access functions below

// GetSettings returns the loaded settings. Though not required, this should be called after LoadSettings
// has been called. Otherwise, a zero/fresh instance of Settings will be returned
func GetSettings() Settings {
	return localSettings
}

// SetActiveServer sets the active server. Updates the settings
func SetActiveServer(serverUUID string) {
	localSettings.ActiveServerUUID = &serverUUID
	WriteSettings()
}

// ActiveServerUUID is a helper to access the setting's ActiveServerUUID
func ActiveServerUUID() string {
	val := GetSettings().ActiveServerUUID
	if val == nil {
		return ""
	}
	return *val
}

func HistoryForServer(uuid string) ServerSettingHistory {
	val, _ := cachedServerHistory[ActiveServerUUID()]
	return val
}

// LastOperation is a shorthand for retriving the settings, finding the appropriate server history,
// and retrieve the last used operation slug
func LastOperation() string {
	val, _ := cachedServerHistory[ActiveServerUUID()]
	return val.LastOperationSlug
}

func SetLastUsedOperation(lastOpSlug string) {
	val, ok := cachedServerHistory[ActiveServerUUID()]
	if !ok {
		val = ServerSettingHistory{
			ServerUUID: ActiveServerUUID(),
		}
	}
	val.LastOperationSlug = lastOpSlug
	cachedServerHistory[ActiveServerUUID()] = val
	WriteSettings() // best effort
}
