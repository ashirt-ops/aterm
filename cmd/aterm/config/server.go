package config

import (
	"encoding/json"
	"io/ioutil"
	"os"
	"sort"

	"github.com/google/uuid"
	"github.com/theparanoids/aterm/common"
	"github.com/theparanoids/aterm/errors"
	"github.com/theparanoids/aterm/network"
)

var knownServers map[string]common.Server
var loadedConnections ServerConnections

// ServerConnections holds the structure for a
type ServerConnections struct {
	Loaded  bool
	Version int64           `json:"version"`
	Servers []common.Server `json:"servers"`
}

// LoadServersFile reads the ATerm servers file, and, if no errors occur, loads it into memory.
func LoadServersFile() error {
	conns, err := loadServers(ATermServersPath())
	if errors.Is(err, os.ErrNotExist) {
		loadedConnections = createDefaultServerConnections()
		writeServers(loadedConnections) // best effort -- it's okay if the file doesn't get written
	} else if err != nil {
		return err
	} else {
		loadedConnections = conns
	}

	setKnownServers(conns)
	return nil
}

func createDefaultServerConnections() ServerConnections {
	return ServerConnections{
		Servers: []common.Server{
			createDefaultServer(),
		},
	}
}

func createDefaultServer() common.Server {
	return common.Server{
		ID:         1,
		ServerName: "default",
		ServerUUID: common.DefaultServerUUID,
	}
}

// GetDefaultConnection retrieves the connection associated with the default connection uuid
// If this server has been deleted, a default/"empty" version of this is returned instead
func GetDefaultConnection() common.Server {
	conn, ok := knownServers[common.DefaultServerUUID]
	if !ok {
		return createDefaultServer()
	}
	return conn
}

// LoadASHIRTServersFile loads the data associated with the ASHIRT "servers" file
func LoadASHIRTServersFile() (ServerConnections, error) {
	return loadServers(ASHIRTServersPath())
}

func loadServers(filepath string) (ServerConnections, error) {
	content, err := ioutil.ReadFile(filepath)
	if err != nil {
		return ServerConnections{}, err
	}

	var serversFile ServerConnections
	err = json.Unmarshal(content, &serversFile)
	if err == nil {
		serversFile.Loaded = true
	}
	return serversFile, err
}

// setKnownServers updates the in-memory representation of the servers data with the provided
// ServerConnections's Servers
func setKnownServers(connections ServerConnections) {
	knownServers = map[string]common.Server{}
	for _, server := range connections.Servers {
		knownServers[server.ServerUUID] = server
	}
}

// SetServer sets the server to the server used on the last run, if it exists. Otherwise,
// no change is made. Returns true if the server was set, false otherwise
// Note: This should really only be called once, on startup, after the config has been loaded
func SetServer(serverUUID string) bool {
	if s, ok := knownServers[serverUUID]; ok {
		network.SetServer(s)
		return true
	}
	return false
}

// GetAlphaSortedServers retrieves the loaded servers, sorts them by name (alphabetical asc), and returns
// the result. Note that this will be done on every execution, so avoid calling repetatively
func GetAlphaSortedServers() []common.Server {
	return getSortedServers(func(a, b common.Server) bool {
		return a.ServerName < b.ServerName
	})
}

// GetIDSortedServers retrieves the loaded servers, sorts them by id (asc), and returns
// the result. Note that this will be done on every execution, so avoid calling repetatively
func GetIDSortedServers() []common.Server {
	return getSortedServers(func(a, b common.Server) bool {
		return a.ID < b.ID
	})
}

func getSortedServers(compareServers func(a, b common.Server) bool) []common.Server {
	foundServers := make([]common.Server, 0, len(knownServers))
	for _, server := range knownServers {
		if server == common.NoServer {
			continue // skip any servers that are fake
		}
		foundServers = append(foundServers, server)
	}
	sort.Slice(foundServers, func(i, j int) bool {
		return compareServers(foundServers[i], foundServers[j])
	})

	return foundServers
}

// GetServer retrives the server definition associated with the given uuid
func GetServer(uuid string) common.Server {
	if s, ok := knownServers[uuid]; ok {
		return s
	}
	return common.NoServer
}

// GetServerCount returns the number of servers currently known to the application
func GetServerCount() int {
	i := 0
	// filter out non-servers
	for _, v := range knownServers {
		if v != common.NoServer {
			i++
		}
	}
	return i
}

// GetCurrentServer is a helper to retrieve the server associated with the CurrenServerUUID.
// Equivalent to GetServer(ActiveServerUUID())
func GetCurrentServer() common.Server {
	return GetServer(ActiveServerUUID())
}

// writeServers writes the given ServerConnections to a file
func writeServers(connections ServerConnections) error {
	data, err := json.Marshal(connections)
	if err != nil {
		return errors.Wrap(err, "Unable to encode servers")
	}
	return errors.MaybeWrap(writeFile(data, ATermServersPath()), "Unable to write servers file")
}

// UpsertServer adds or updates the servers definition (if possible). If the ServerUUID is found
// then an update occurs. Otherwise, a new entry is added.
func UpsertServer(server common.Server) (common.Server, error) {
	if server.IsValidServer() {
		if _, ok := knownServers[server.ServerUUID]; !ok {
			server = createServer(server)
		} else {
			if err := updateServer(server); err != nil {
				return server, err
			}
		}

		return server, writeServers(loadedConnections)
	}
	return server, errors.ErrInvalidServer
}

// ImportServersConnections performs a batch insert of servers
func ImportServersConnections(conns ServerConnections, replace bool) error {
	for _, server := range conns.Servers {
		if !server.IsValidServer() { // this should never happen, but just to be safe...
			continue
		}

		if _, ok := knownServers[server.ServerUUID]; !ok { // i.e. this is a new server
			createServerKeepUUID(server)
		} else if replace {
			updateServer(server) // guaranteed to work -- we already know we have the server
		}

	}
	return writeServers(loadedConnections)
}

// createServer adds a new server to the list of servers, but, notably, does not write the entry to disk.
// Note that this adds an entry to both the loadedConnections ServerConnection, and the knownServers map
func createServer(server common.Server) common.Server {
	server.ServerUUID = uuid.New().String()
	return createServerKeepUUID(server)
}

// createServerKeepUUID adds a new server to the list of servers. Unlike createServer, the uuid provided
// by the object is retained. Useful for imports, where new IDs need to be specified
func createServerKeepUUID(server common.Server) common.Server {
	sortedServers := GetIDSortedServers()

	server.ID = sortedServers[len(sortedServers)-1].ID + 1
	sortedServers = append(sortedServers, server)

	knownServers[server.ServerUUID] = server
	loadedConnections.Servers = sortedServers
	return server
}

// updateServer updates an existing server to the list of servers, but does not write the
// entry to disk. Useful in situations where multiple inserts occur
// Note that this adds an entry to both the loadedConnections ServerConnection, and the knownServers map
func updateServer(server common.Server) error {
	if _, ok := knownServers[server.ServerUUID]; !ok {
		return errors.ErrServerNotFound
	}
	knownServers[server.ServerUUID] = server
	loadedConnections.Servers = GetIDSortedServers()
	return nil
}

// DeleteServer removes a server from the list of known servers
func DeleteServer(serverUUID string) error {
	if _, ok := knownServers[serverUUID]; !ok {
		return errors.ErrServerNotFound
	}
	knownServers[serverUUID] = common.NoServer
	loadedConnections.Servers = GetIDSortedServers()
	return writeServers(loadedConnections)
}
