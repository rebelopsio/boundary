package infrastructure

import (
	"github.com/example/fr24/services/auth/domain"
	"github.com/example/fr24/shared/logger"
)

// AuthRepo implements domain.UserRepository.
type AuthRepo struct {
	log logger.Logger
}

func (r *AuthRepo) FindByID(id string) (*domain.User, error) {
	return nil, nil
}
