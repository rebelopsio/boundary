package infrastructure

import (
	"github.com/example/rcm-ddd/domain"
)

// UserRepo is a concrete infrastructure adapter.
type UserRepo struct {
	user domain.User
}
