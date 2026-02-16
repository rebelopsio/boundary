package user

import (
	"github.com/example/app/internal/domain/user"
)

// UserService is an application use case
type UserService struct {
	repo user.UserRepository
}

func NewUserService(repo user.UserRepository) *UserService {
	return &UserService{repo: repo}
}
