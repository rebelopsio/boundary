package infrastructure

import (
	"github.com/example/fr19-cross-cutting/domain"
	"github.com/example/fr19-cross-cutting/pkg/logger"
)

// PostgresRepo implements domain.UserRepository.
type PostgresRepo struct {
	log logger.Logger
}

func (r *PostgresRepo) FindByID(id string) (*domain.User, error) {
	return nil, nil
}
