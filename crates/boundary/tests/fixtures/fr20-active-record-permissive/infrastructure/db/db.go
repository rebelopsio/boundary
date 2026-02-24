package db

// Connection is a database connection handle.
type Connection struct {
	DSN string
}

func (c *Connection) Exec(query string, args ...interface{}) error {
	return nil
}
