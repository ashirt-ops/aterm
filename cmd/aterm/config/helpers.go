package config

import (
	"io/ioutil"
	"os"
	"path"

	"github.com/theparanoids/aterm/errors"
	"gopkg.in/yaml.v2"
)

func writeFile(data []byte, filePath string) error {
	err := os.MkdirAll(path.Dir(filePath), 0755)
	if err != nil {
		return err
	}
	return ioutil.WriteFile(filePath, data, 0664)
}

func readYamlConfig(container interface{}, path string, makeDefault func()) error {
	data, err := ioutil.ReadFile(path)

	if errors.Is(err, os.ErrNotExist) {
		makeDefault()
	} else if err != nil {
		return errors.Wrap(err, "Unable to read config file")
	} else {
		if err = yaml.Unmarshal(data, container); err != nil {
			return errors.Wrap(err, "Unable to interpret config file as YAML document")
		}
	}

	return nil
}
