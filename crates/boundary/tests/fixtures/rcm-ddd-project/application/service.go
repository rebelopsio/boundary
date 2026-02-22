package application

import (
	"github.com/example/rcm-ddd/domain"
)

// UserService is a concrete application service.
type UserService struct {
	repo domain.UserRepository
}
