package main

import (
	"io"
)

var gitHash string

// GitHash returns the compiled in hash, or "(unknown)" if the value was not specified
func GitHash() string {
	if gitHash == "" {
		return "(unknown)"
	}
	return gitHash
}

// PrintVersion writes the version info to the provided writer (normally os.Stdout)
func PrintVersion(writer io.Writer) {
	info :=
		"ATerm\n\r" +
			"Version Info:\n\r" +
			"  Hash: " + GitHash() + "\n\r"

	writer.Write([]byte(info))
}
