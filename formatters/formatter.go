// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package formatters

import "github.com/theparanoids/aterm/common"

// Formatter is a small interface for building up a file (in the form of bytes). The separation
// allows for streaming parts of a file
type Formatter interface {
	WriteHeader(Metadata) ([]byte, error)
	WriteFooter(Metadata) ([]byte, error)
	WriteEvent(evt common.Event) ([]byte, error)
}
