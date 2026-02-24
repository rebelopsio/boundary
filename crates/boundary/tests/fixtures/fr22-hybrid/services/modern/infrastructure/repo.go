package infrastructure

import "github.com/example/fr22/services/modern/domain"

// PostgresRepo implements domain.UserRepository.
type PostgresRepo struct{}

func (r *PostgresRepo) FindByID(id string) (*domain.User, error) {
	return nil, nil
}
