// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package network

import (
	"testing"

	"github.com/stretchr/testify/require"
)

func TestSetBaseURL(t *testing.T) {
	require.False(t, IsServerSet())
	SetBaseURL("Something")
	require.Equal(t, "Something/api", apiURL)
	require.True(t, IsServerSet())
}
