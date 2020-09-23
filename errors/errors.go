package errors

import (
	"errors"
	"fmt"

	"github.com/hashicorp/go-multierror"
)

// Wrap constructs a new error from the provided error with the msg text applied
// the wrapped message is in the form of "msg : err". if the provided error is nil, then
// errors.New is used instead
func Wrap(err error, msg string) error {
	if err == nil {
		return New(msg)
	}
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

// New wraps golang's error.New function to provide easier use when using this package and trying to
// create a new error
func New(msg string) error {
	return errors.New(msg)
}

// Is wraps golang's errors.Is function to provide easier use when using this package and golang's
// underlying errors package
func Is(err, target error) bool {
	return errors.Is(err, target)
}

// MultiErrorPrintFormat provides the common error printing function for hashicorp/go-multierror
// Format is err1 : err2 : ... : errN (roughly equivalement to strings.Join(errs, " : ") )
func MultiErrorPrintFormat(errs []error) string {
	errString := ""
	if len(errs) > 0 {
		errString += errs[0].Error()
	}
	for i := 1; i < len(errs); i++ {
		errString += " : " + errs[i].Error()
	}
	return errString
}

// Append combines the provided errors into a single error. Any nil-value error will be effectively
// dropped (since it's not an error), and if all errors are nil, then nil is returned
func Append(e1 error, e2 ...error) error {
	merged := multierror.Append(e1, e2...)
	merged.ErrorFormat = MultiErrorPrintFormat
	return merged.ErrorOrNil()
}
