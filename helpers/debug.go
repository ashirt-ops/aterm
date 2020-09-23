package helpers

import (
	"fmt"
	"os"
	"strings"
)

var enableDebugPrint = true

var debugPrintf func(msg string, vals ...interface{})
var debugPrintln func(parts ...string)
var debugPrint func(parts ...string)

func init() {
	if enableDebugPrint {
		debugPrint = func(parts ...string) { os.Stderr.WriteString(strings.Join(parts, " ")) }
		debugPrintln = func(parts ...string) { debugPrint(strings.Join(parts, " "), "\n") }
		debugPrintf = func(msg string, vals ...interface{}) { debugPrint(fmt.Sprintf(msg, vals...)) }
	} else {
		debugPrint = func(parts ...string) {}
		debugPrintln = func(parts ...string) {}
		debugPrintf = func(msg string, vals ...interface{}) {}
	}
}

func DebugPrintf(msg string, vals ...interface{}) {
	debugPrintf(msg, vals...)
}

func DebugPrintln(parts ...string) {
	debugPrintln(parts...)
}

func DebugPrint(s string) {
	debugPrint(s)
}
