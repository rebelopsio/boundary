package infrastructure

import (
	"github.com/example/interface-coverage/domain"
)

// PostgresUserRepository is the one concrete adapter.
type PostgresUserRepository struct {
	db interface{}
}

func (r *PostgresUserRepository) FindByID(id string) (*domain.User, error) {
	return nil, nil
}

func (r *PostgresUserRepository) Save(user *domain.User) error {
	return nil
}
