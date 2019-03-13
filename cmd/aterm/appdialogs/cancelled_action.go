// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package appdialogs

// CanceledOperation is an error where the receiver can view the data/action that was cancelled
type CanceledOperation struct {
	Data interface{}
}

func (CanceledOperation) Error() string {
	return "User Cancelled"
}
