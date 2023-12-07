package write

import (
	"github.com/theparanoids/aterm/common"
	"github.com/theparanoids/aterm/formatters"
)

// TerminalWriter is a small interface into, essentially, a formatters.Formatter and an io.Writer
// The TerminalWriter is responsible for handling
type TerminalWriter interface {
	WriteHeader(formatters.Metadata)
	WriteFooter(formatters.Metadata)
	WriteEvent(common.Event)
}
