package infrastructure

import (
	"github.com/example/fr-go-adapters/domain"
)

// mongoUserRepository is an unexported infrastructure adapter implementing domain.UserRepository.
// Go convention: concrete adapter type is unexported; the constructor is exported.
type mongoUserRepository struct {
	client interface{}
}

func NewMongoUserRepository() domain.UserRepository {
	return &mongoUserRepository{}
}

func (r *mongoUserRepository) FindByID(id string) (*domain.User, error) {
	return nil, nil
}

func (r *mongoUserRepository) Save(user *domain.User) error {
	return nil
}
