package write

import (
	"io/ioutil"
	"os"
	"path/filepath"
)

// NewFile creates and opens a file at the named location, or name is empty, creates a new
// temporary file at the indicated dir (if dir is also empty, will be stored under the os temp directory).
// This file will be prefixed with "recording_". Under the hood, uses ioutil.TempFile in this case.
func NewFile(dir, name string) (*os.File, error) {
	var realFile *os.File
	err := os.MkdirAll(dir, 0775)
	if err != nil {
		return nil, err
	}

	if name == "" {
		realFile, err = ioutil.TempFile(dir, "recording_*.cast")
	} else {
		filename := filepath.Join(dir, name)
		realFile, err = os.OpenFile(filename, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0644)
	}

	return realFile, err
}
