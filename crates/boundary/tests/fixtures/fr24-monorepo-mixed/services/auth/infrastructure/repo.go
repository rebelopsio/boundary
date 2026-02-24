package infrastructure

import "example.com/app/services/auth/domain"

// InMemoryUserRepository is a simple in-memory implementation.
type InMemoryUserRepository struct {
	users map[string]*domain.User
}

func (r *InMemoryUserRepository) FindByID(id string) (*domain.User, error) {
	return r.users[id], nil
}
