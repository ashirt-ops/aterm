package recorders

import (
	"time"

	"github.com/theparanoids/aterm/common"
	"github.com/theparanoids/aterm/write"
)

// Recorder is an interface for tracking I/O events
type Recorder interface {
	AddEvent(common.EventType, string, time.Time)
	GetEventCount() int
	GetDurationInSeconds() float64
	GetStartTime() int64
	Output(write.TerminalWriter)
}
