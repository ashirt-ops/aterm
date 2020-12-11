package config

import "gopkg.in/yaml.v2"

type TermRecorderConfigV0 struct {
	Interpreted   bool
	ConfigVersion int64 `yaml:"configVersion"` // could be anything
}

func interpretConfigData(data []byte) TermRecorderConfigV0 {
	var basicConfig TermRecorderConfigV0
	err := yaml.Unmarshal(data, &basicConfig)
	if err != nil {
		return TermRecorderConfigV0{}
	}
	basicConfig.Interpreted = true
	return basicConfig
}
