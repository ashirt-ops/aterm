package errors

import (
	"fmt"
)

// Wrap constructs a new error from the provided error with the msg text applied
func Wrap(err error, msg string) error {
	return fmt.Errorf("%v : %w", msg, err)
}
