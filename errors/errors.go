package errors

import (
	"fmt"
)

// Wrap constructs a new error from the provided error with the msg text applied
// the wrapped message is in the form of "msg : err"
func Wrap(err error, msg string) error {
	return fmt.Errorf("%v : %w", msg, err)
}

// MaybeWrap conditionally wraps an error. if the provided error is nil, then nil will be returned
// otherwise, the error will be wrapped as per the errors.Wrap function
func MaybeWrap(err error, msg string) error {
	if err != nil {
		return Wrap(err, msg)
	}
	return nil
}
