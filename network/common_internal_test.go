// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package network

import (
	"fmt"
	"testing"

	"github.com/stretchr/testify/require"
	"github.com/theparanoids/aterm/common"
)

func MkTestServer(port string) common.Server {
	t := common.Server{
		ID:         100,
		ServerName: "TestServer",
		ServerUUID: "00000000-0000-4000-0000-000000000000",
		AccessKey:  "1",
		SecretKey:  "1",
		HostPath:   fmt.Sprintf("http://localhost%v", port),
	}
	return t
}

func TestSetServer(t *testing.T) {
	testServer := MkTestServer(":8001")
	require.False(t, IsServerSet())
	SetServer(testServer)
	require.True(t, IsServerSet())
	require.Equal(t, currentServer, testServer)
}
