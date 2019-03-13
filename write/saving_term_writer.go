// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package write

import (
	"github.com/theparanoids/aterm/common"
	"github.com/theparanoids/aterm/formatters"
)

// SaveTermWriter records the messages sent to it. For testing purposes only
type SaveTermWriter struct {
	HeaderMetadata *formatters.Metadata
	FooterMetadata *formatters.Metadata
	AllEvents      *[]common.Event
}

// NewSaveTermWrier is a constructor for SaveTermWriter
func NewSaveTermWrier() SaveTermWriter {
	headerBuff := formatters.Metadata{}
	footerBuff := formatters.Metadata{}
	eventBuff := make([]common.Event, 0)
	return SaveTermWriter{
		HeaderMetadata: &headerBuff,
		AllEvents:      &eventBuff,
		FooterMetadata: &footerBuff,
	}
}

// WriteHeader saves the header to an internal buffer (HeaderMetadata)
func (fw SaveTermWriter) WriteHeader(m formatters.Metadata) {
	*fw.HeaderMetadata = m
}

// WriteFooter saves the footer to an internal buffer (FooterMetadata)
func (fw SaveTermWriter) WriteFooter(m formatters.Metadata) {
	*fw.FooterMetadata = m
}

// WriteEvent saves the event to an internal buffer (AllEvents)
func (fw SaveTermWriter) WriteEvent(evt common.Event) {
	*fw.AllEvents = append(*fw.AllEvents, evt)
}
