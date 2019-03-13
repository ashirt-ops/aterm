// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package isthere

import (
	"fmt"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestNo(t *testing.T) {
	assert.True(t, No(nil))
	assert.False(t, No(fmt.Errorf("yep")))
}
