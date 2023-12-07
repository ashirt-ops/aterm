package dialog

import (
	"fmt"
	"strings"
	"sync"
	"time"

	"github.com/theparanoids/aterm/fancy"
)

// ShowLoadingAnimation presents the given text plus a looping dot animation.
// Should be called as a goroutine, otherwise this is likely to be an infinite loop
//
// # To stop, set the stopCheck parameter to true
//
// A common solution can be used with the DoBackgroundLoading function
func ShowLoadingAnimation(text string, stopCheck *bool) {
	count := 0
	max := 4
	dots := func() string {
		return strings.Repeat(".", count)
	}
	for {
		if !*stopCheck {
			fmt.Print(fancy.ClearLine(text+dots(), 0))
			count = (count + 1) % max
		}
		time.Sleep(500 * time.Millisecond)
	}
}

// SyncedFunc wraps the provided function in a wait group, and sends a Done signal
// once the provided function completes
func SyncedFunc(fn func()) func(*sync.WaitGroup) {
	return func(wg *sync.WaitGroup) {
		fn()
		wg.Done()
	}
}

// DoBackgroundLoading presents the loading animation in a separate goroutine, and executes any number
// of other functions in their own goroutines in the background. Loading will complete once ALL
// of the passed funcs complete. See SyncedFunc to help generate appropriate functions
func DoBackgroundLoading(funcs ...func(wg *sync.WaitGroup)) {
	DoBackgroundLoadingWithMessage("Loading", funcs...)
}

// DoBackgroundLoadingWithMessage is identical to DoBackgroundLoading, except the given message
// will be disabled instead of "Loading"
func DoBackgroundLoadingWithMessage(msg string, funcs ...func(wg *sync.WaitGroup)) {
	var wg sync.WaitGroup
	wg.Add(len(funcs))
	for _, f := range funcs {
		go f(&wg)
	}
	stop := false
	go ShowLoadingAnimation(msg, &stop)
	wg.Wait()
	stop = true
	fmt.Print(fancy.ClearLine(""))
}
