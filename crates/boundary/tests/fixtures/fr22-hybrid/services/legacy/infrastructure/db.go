package infrastructure

// DB is the legacy database handle.
type DB struct {
	DSN string
}

func Exec(query string) error {
	return nil
}
