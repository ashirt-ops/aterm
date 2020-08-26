// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package dialog

import (
	"io"

	"github.com/manifoldco/promptui"
)

// MkBasicSelect provides a base for any Select operation. This essentially
// ensures that the given Select struct will read input from the proper source
func MkBasicSelect(inputStream io.ReadCloser) promptui.Select {
	return promptui.Select{
		Stdin:             inputStream,
		StartInSearchMode: false,
	}
}
