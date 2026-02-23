package application

import (
	"github.com/example/pattern-ddd/domain"
)

// UserService is a concrete application service.
type UserService struct {
	repo domain.UserRepository
}
