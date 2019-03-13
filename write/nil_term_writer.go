// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package write

import (
	"github.com/theparanoids/aterm/common"
	"github.com/theparanoids/aterm/formatters"
)

// NilTermWriter ignores all write events and outputs nothing.
type NilTermWriter struct{}

// WriteHeader does nothing
func (fw NilTermWriter) WriteHeader(m formatters.Metadata) {
}

// WriteFooter does nothing
func (fw NilTermWriter) WriteFooter(m formatters.Metadata) {
}

// WriteEvent does nothing
func (fw NilTermWriter) WriteEvent(evt common.Event) {
}
