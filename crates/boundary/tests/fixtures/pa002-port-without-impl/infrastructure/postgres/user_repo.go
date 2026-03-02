package postgres

import "github.com/example/pa002/domain/ports"

type postgresUserRepository struct {
	connStr string
}

// NewPostgresUserRepository creates a new postgres user repository.
func NewPostgresUserRepository(connStr string) ports.UserRepository {
	return &postgresUserRepository{connStr: connStr}
}
