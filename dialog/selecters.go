// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package dialog

import (
	"io"
	"os"

	"github.com/manifoldco/promptui"
)

// MkBasicSelect provides a base for any Select operation. This essentially
// ensures that the given Select struct will read input from the proper source
func MkBasicSelect(inputStream io.ReadCloser) promptui.Select {
	return promptui.Select{
		Stdin:             inputStream,
		StartInSearchMode: false,
		Stdout:            &bellSkipper{},
	}
}

// From: https://github.com/manifoldco/promptui/issues/49#issuecomment-573814976
// bellSkipper implements an io.WriteCloser that skips the terminal bell
// character (ASCII code 7), and writes the rest to os.Stderr. It is used to
// replace readline.Stdout, that is the package used by promptui to display the
// prompts.
//
// This is a workaround for the bell issue documented in
// https://github.com/manifoldco/promptui/issues/49.
type bellSkipper struct{}

// Write implements an io.WriterCloser over os.Stderr, but it skips the terminal
// bell character.
func (bs *bellSkipper) Write(b []byte) (int, error) {
	const charBell = 7 // c.f. readline.CharBell
	if len(b) == 1 && b[0] == charBell {
		return 0, nil
	}
	return os.Stderr.Write(b)
}

// Close implements an io.WriterCloser over os.Stderr.
func (bs *bellSkipper) Close() error {
	return os.Stderr.Close()
}
