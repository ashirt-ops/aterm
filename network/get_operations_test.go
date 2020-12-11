// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package network_test

import (
	"encoding/json"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/theparanoids/ashirt-server/backend/dtos"
	"github.com/theparanoids/aterm/network"
)

func TestGetOperations(t *testing.T) {
	t.Skip("skipping network tests")
	op1Raw := `{"slug": "s1", "name": "Jack", "numUsers": 1024, "status": 7, "id": 3}`
	op2Raw := `{"slug": "s2", "name": "Jill", "numUsers": 2048, "status": 2, "id": 10}`
	resp := "[" + op1Raw + "," + op2Raw + "]"
	makeServer(Route{"GET", "/api/operations", newCannedResponse(200, resp)})
	network.SetServer(network.MkTestServer(testPort))

	ops, err := network.GetOperations()
	var op1, op2 dtos.Operation
	json.Unmarshal([]byte(op1Raw), &op1)
	json.Unmarshal([]byte(op2Raw), &op2)

	assert.Nil(t, err)
	assert.Equal(t, len(ops), 2)
	assert.Equal(t, ops[0], op1)
	assert.Equal(t, ops[1], op2)
}
